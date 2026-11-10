# Brevi's pydantic: the part of pydantic 2.13 (and of pydantic-core 2.46, its Rust validator) that programs use
# most, written in Python and built natively with the program: BaseModel, Field, ConfigDict, field_validator,
# model_validator, computed_field, ValidationError and the JSON of model_dump_json / model_validate_json. Its
# messages, error locations, reprs and JSON are pydantic's own, byte for byte.
#
# pydantic reads a model's annotations as Python's typing objects. Brevi builds no typing objects: the compiler
# gives each class of a program that imports pydantic an `__annotations__` of descriptors instead (tuples such as
# ('list', ('int',), 'list', False), see src/dynpy/pyd.rs), and checks when the program is built that every
# annotation of every model is one this copy has (TypeAdapter's type too). What it lacks (generic models, datetime,
# UUID, serialization-mode JSON schema, serializers, discriminated unions, ...) is refused when the program is built
# (src/caps.rs), and CPython's pydantic runs such a program.

import json as _json
import math as _math
import re as _re

VERSION = "2.13.5"
__version__ = VERSION
_URL = "https://errors.pydantic.dev/2.13/v/"


class _Undefined:
    def __repr__(self):
        return "PydanticUndefined"

    def __bool__(self):
        return False


PydanticUndefined = _Undefined()

# ---------------------------------------------------------------- errors

_MSG = {
    "missing": "Field required",
    "frozen_field": "Field is frozen",
    "frozen_instance": "Instance is frozen",
    "needs_python_object": "Cannot check `isinstance` when validating from json, use a JsonOrPython validator instead",
    "extra_forbidden": "Extra inputs are not permitted",
    "model_attributes_type": "Input should be a valid dictionary or object to extract fields from",
    "none_required": "Input should be None",
    "finite_number": "Input should be a finite number",
    "string_type": "Input should be a valid string",
    "string_unicode": "Input should be a valid string, unable to parse raw data as a unicode string",
    "dict_type": "Input should be a valid dictionary",
    "list_type": "Input should be a valid list",
    "tuple_type": "Input should be a valid tuple",
    "set_type": "Input should be a valid set",
    "set_item_not_hashable": "Set items should be hashable",
    "frozen_set_type": "Input should be a valid frozenset",
    "bool_type": "Input should be a valid boolean",
    "bool_parsing": "Input should be a valid boolean, unable to interpret input",
    "int_type": "Input should be a valid integer",
    "int_parsing": "Input should be a valid integer, unable to parse string as an integer",
    "int_parsing_size": "Unable to parse input string as an integer, exceeded maximum size",
    "int_from_float": "Input should be a valid integer, got a number with a fractional part",
    "float_type": "Input should be a valid number",
    "float_parsing": "Input should be a valid number, unable to parse string as a number",
    "bytes_type": "Input should be a valid bytes",
    "json_type": "JSON input should be string, bytes or bytearray",
}


def _num(v):
    # a number in a message, as pydantic-core's Rust Display writes it (no exponent, 1.0 as 1)
    if isinstance(v, bool) or not isinstance(v, float):
        return str(v)
    if v != v:
        return "NaN"
    if v in (float("inf"), float("-inf")):
        return "inf" if v > 0 else "-inf"
    r = repr(v)
    if "e" not in r:
        return r[:-2] if r.endswith(".0") else r
    m, e = r.split("e")
    neg = m.startswith("-")
    if neg:
        m = m[1:]
    if "." in m:
        a, b = m.split(".")
    else:
        a, b = m, ""
    digits = a + b
    point = len(a) + int(e)
    if point <= 0:
        s = "0." + "0" * (-point) + digits
    elif point >= len(digits):
        s = digits + "0" * (point - len(digits))
    else:
        s = digits[:point] + "." + digits[point:]
    return ("-" if neg else "") + s


def _plural(n, word):
    return word if n == 1 else word + "s"


def _message(t, ctx):
    m = _MSG.get(t)
    if m is not None:
        return m
    c = ctx or {}
    if t == "no_such_attribute":
        return "Object has no attribute '%s'" % c["attribute"]
    if t == "greater_than":
        return "Input should be greater than " + _num(c["gt"])
    if t == "greater_than_equal":
        return "Input should be greater than or equal to " + _num(c["ge"])
    if t == "less_than":
        return "Input should be less than " + _num(c["lt"])
    if t == "less_than_equal":
        return "Input should be less than or equal to " + _num(c["le"])
    if t == "multiple_of":
        return "Input should be a multiple of " + _num(c["multiple_of"])
    if t == "too_short":
        n = c["min_length"]
        return "%s should have at least %d %s after validation, not %d" % (c["field_type"], n, _plural(n, "item"), c["actual_length"])
    if t == "too_long":
        n = c["max_length"]
        return "%s should have at most %d %s after validation, not %d" % (c["field_type"], n, _plural(n, "item"), c["actual_length"])
    if t == "string_too_short":
        n = c["min_length"]
        return "String should have at least %d %s" % (n, _plural(n, "character"))
    if t == "string_too_long":
        n = c["max_length"]
        return "String should have at most %d %s" % (n, _plural(n, "character"))
    if t == "string_pattern_mismatch":
        return "String should match pattern '%s'" % c["pattern"]
    if t == "bytes_too_short":
        n = c["min_length"]
        return "Data should have at least %d %s" % (n, _plural(n, "byte"))
    if t == "bytes_too_long":
        n = c["max_length"]
        return "Data should have at most %d %s" % (n, _plural(n, "byte"))
    if t == "enum" or t == "literal_error":
        return "Input should be " + c["expected"]
    if t == "is_instance_of":
        return "Input should be an instance of " + c["class"]
    if t == "model_type":
        return "Input should be a valid dictionary or instance of " + c["class_name"]
    if t == "value_error":
        return "Value error, " + str(c["error"])
    if t == "assertion_error":
        return "Assertion failed, " + str(c["error"])
    if t == "json_invalid":
        return "Invalid JSON: " + c["error"]
    return t


class _Line:
    # one error: its type, location (a list, outermost first), message, input and context
    def __init__(self, t, inp, ctx=None, msg=None, custom=False):
        self.t = t
        self.loc = []
        self.inp = inp
        self.ctx = ctx
        self.msg = _message(t, ctx) if msg is None else msg
        self.custom = custom


class _Err(Exception):
    # validation failed: the errors, as a list of _Line
    def __init__(self, lines):
        self.lines = lines


def _fail(t, inp, ctx=None):
    raise _Err([_Line(t, inp, ctx)])


def _at(lines, key):
    for ln in lines:
        ln.loc.insert(0, key)
    return lines


def _trunc(r):
    # pydantic-core cuts a long repr to its first 25 and last 24 bytes, at character boundaries
    b = r.encode("utf-8")
    if len(b) <= 50:
        return r
    i = 25
    while i > 0 and (b[i] & 0xC0) == 0x80:
        i -= 1
    j = len(b) - 24
    while j < len(b) and (b[j] & 0xC0) == 0x80:
        j += 1
    return b[:i].decode("utf-8") + "..." + b[j:].decode("utf-8")


class PydanticCustomError(ValueError):
    def __init__(self, error_type, message_template, context=None):
        self._t = error_type
        self._tmpl = message_template
        self._ctx = context

    @property
    def type(self):
        return self._t

    @property
    def message_template(self):
        return self._tmpl

    @property
    def context(self):
        return self._ctx

    def message(self):
        m = self._tmpl
        if self._ctx:
            for k, v in self._ctx.items():
                m = m.replace("{" + k + "}", str(v))
        return m

    def __str__(self):
        return self.message()

    def __repr__(self):
        return "%s [type=%s, context=%r]" % (self.message(), self._t, self._ctx)


class ValidationError(ValueError):
    def __init__(self, title, lines, hide_input=False):
        self._title = title
        self._lines = lines
        self._hide = hide_input

    @property
    def title(self):
        return self._title

    def error_count(self):
        return len(self._lines)

    def errors(self, *, include_url=True, include_context=True, include_input=True):
        out = []
        for ln in self._lines:
            d = {"type": ln.t, "loc": tuple(ln.loc), "msg": ln.msg}
            if include_input:
                d["input"] = ln.inp
            if include_context and ln.ctx is not None:
                d["ctx"] = ln.ctx
            if include_url and not ln.custom:
                d["url"] = _URL + ln.t
            out.append(d)
        return out

    def __str__(self):
        n = len(self._lines)
        parts = ["%d validation error%s for %s" % (n, "" if n == 1 else "s", self._title)]
        for ln in self._lines:
            if ln.loc:
                parts.append(".".join(("`%s`" % x) if (isinstance(x, str) and "." in x) else str(x) for x in ln.loc))
            if self._hide:
                parts.append("  %s [type=%s]" % (ln.msg, ln.t))
            else:
                parts.append("  %s [type=%s, input_value=%s, input_type=%s]" % (ln.msg, ln.t, _trunc(repr(ln.inp)), type(ln.inp).__name__))
            if not ln.custom:
                parts.append("    For further information visit " + _URL + ln.t)
        return "\n".join(parts)

    def __repr__(self):
        return str(self)

    def json(self, *, indent=None, include_url=True, include_context=True, include_input=True):
        errs = self.errors(include_url=include_url, include_context=include_context, include_input=include_input)
        for e in errs:
            e["loc"] = list(e["loc"])
            if "ctx" in e:
                e["ctx"] = {k: (str(v) if isinstance(v, BaseException) else v) for k, v in e["ctx"].items()}
        return _jdump(_to_jsonable(errs), indent)


ValidationError.__module__ = "pydantic_core._pydantic_core"
PydanticCustomError.__module__ = "pydantic_core._pydantic_core"


class PydanticUserError(TypeError):
    def __init__(self, message, *, code=None):
        self.message = message
        self.code = code

    def __str__(self):
        return self.message


class PydanticSchemaGenerationError(PydanticUserError):
    pass


# ---------------------------------------------------------------- Field, config, decorators


class _Meta:
    # a constraint of a field: Gt(gt=1), MinLen(min_length=2) ...
    def __init__(self, name, key, value):
        self.name = name
        self.key = key
        self.value = value

    def __repr__(self):
        return "%s(%s=%r)" % (self.name, self.key, self.value)


_CONSTRAINTS = [("gt", "Gt"), ("ge", "Ge"), ("lt", "Lt"), ("le", "Le"), ("multiple_of", "MultipleOf"), ("min_length", "MinLen"), ("max_length", "MaxLen")]
_GENERAL = ["strict", "allow_inf_nan", "pattern"]


class FieldInfo:
    def __init__(self, default=PydanticUndefined, *, default_factory=None, alias=None, validation_alias=None, serialization_alias=None,
                 title=None, description=None, examples=None, exclude=None, repr=True, frozen=None, validate_default=None,
                 json_schema_extra=None, gt=None, ge=None, lt=None, le=None, multiple_of=None, min_length=None, max_length=None,
                 pattern=None, strict=None, allow_inf_nan=None, deprecated=None, init=None, kw_only=None, discriminator=None,
                 alias_priority=None, union_mode=None, coerce_numbers_to_str=None, fail_fast=None, field_title_generator=None,
                 max_digits=None, decimal_places=None, init_var=None):
        if default is Ellipsis:
            default = PydanticUndefined
        self.default = default
        self.default_factory = default_factory
        self.alias = alias
        self.validation_alias = validation_alias if validation_alias is not None else alias
        self.serialization_alias = serialization_alias if serialization_alias is not None else alias
        self.alias_priority = 2 if alias is not None else alias_priority
        self.title = title
        self.description = description
        self.examples = examples
        self.exclude = exclude
        self.repr = repr
        self.frozen = frozen
        self.validate_default = validate_default
        self.json_schema_extra = json_schema_extra
        self.deprecated = deprecated
        self.discriminator = discriminator
        self.annotation = None
        self._desc = None
        self.metadata = []
        vals = {"gt": gt, "ge": ge, "lt": lt, "le": le, "multiple_of": multiple_of, "min_length": min_length, "max_length": max_length}
        for k, n in _CONSTRAINTS:
            if vals[k] is not None:
                self.metadata.append(_Meta(n, k, vals[k]))
        general = {"strict": strict, "allow_inf_nan": allow_inf_nan, "pattern": pattern}
        g = [(k, general[k]) for k in _GENERAL if general[k] is not None]
        if g:
            self.metadata.append(_General(g))

    def is_required(self):
        return self.default is PydanticUndefined and self.default_factory is None

    def get_default(self, *, call_default_factory=False):
        if self.default_factory is not None:
            return self.default_factory() if call_default_factory else None
        return self.default

    def _args(self):
        a = [("annotation", _show(self._desc) if self._desc is not None else "NoneType"), ("required", repr(self.is_required()))]
        if self.default is not PydanticUndefined:
            a.append(("default", repr(self.default)))
        if self.default_factory is not None:
            a.append(("default_factory", getattr(self.default_factory, "__name__", repr(self.default_factory))))
        if self.alias is not None:
            a.append(("alias", repr(self.alias)))
            a.append(("alias_priority", repr(self.alias_priority)))
        for k in ["title", "description", "examples", "exclude"]:
            v = getattr(self, k)
            if v is not None:
                a.append((k, repr(v)))
        if self.frozen is not None:
            a.append(("frozen", repr(self.frozen)))
        if not self.repr:
            a.append(("repr", "False"))
        if self.metadata:
            a.append(("metadata", repr(self.metadata)))
        return a

    def __repr__(self):
        return "FieldInfo(" + ", ".join(k + "=" + v for k, v in self._args()) + ")"

    def __str__(self):
        return " ".join(k + "=" + v for k, v in self._args())


class _General:
    def __init__(self, items):
        self.items = items

    def __repr__(self):
        return "_PydanticGeneralMetadata(" + ", ".join("%s=%r" % kv for kv in self.items) + ")"


def Field(default=PydanticUndefined, **kw):
    return FieldInfo(default, **kw)


class ModelPrivateAttr:
    def __init__(self, default=PydanticUndefined, default_factory=None):
        self.default = default
        self.default_factory = default_factory

    def get_default(self):
        if self.default_factory is not None:
            return self.default_factory()
        return _copy_default(self.default)

    def __repr__(self):
        return "ModelPrivateAttr(default=%r)" % (self.default,)


def PrivateAttr(default=PydanticUndefined, *, default_factory=None, init=False):
    return ModelPrivateAttr(default, default_factory)


def ConfigDict(**kw):
    return dict(kw)


class _Dec:
    # what field_validator, model_validator and computed_field leave in a class body
    def __init__(self, kind, f, fields=(), mode="after", extra=None):
        self.kind = kind
        self.f = f
        self.fields = fields
        self.mode = mode
        self.extra = extra or {}


def _unwrap(f):
    # the function under classmethod / staticmethod, and whether it takes cls
    k = getattr(f, "__bv_kind__", None)
    if k is not None:
        return f, (True if k == 2 else (False if k == 1 else None))
    if isinstance(f, classmethod):
        return f.__func__, True
    if isinstance(f, staticmethod):
        return f.__func__, False
    return f, None


def field_validator(field, /, *fields, mode="after", check_fields=None, json_schema_input_type=PydanticUndefined):
    if callable(field) and not isinstance(field, str):
        raise PydanticUserError("`@field_validator` should be used with fields and keyword arguments, not bare. E.g. usage should be `@validator('<field_name>', ...)`", code="validator-no-fields")
    names = (field,) + fields

    def dec(f):
        return _Dec("field", f, names, mode)

    return dec


def model_validator(*, mode):
    def dec(f):
        return _Dec("model", f, (), mode)

    return dec


class _Computed:
    def __init__(self, prop, repr, alias, return_type):
        self.prop = prop
        self.repr = repr
        self.alias = alias
        self.return_type = return_type


def computed_field(func=None, /, *, alias=None, alias_priority=None, title=None, field_title_generator=None, description=None,
                   deprecated=None, examples=None, json_schema_extra=None, repr=True, return_type=PydanticUndefined):
    def dec(f):
        if not _is_prop(f):
            f = property(f)
        return _Dec("computed", f, (), "after", {"repr": repr, "alias": alias})

    if func is None:
        return dec
    return dec(func)


class ValidationInfo:
    def __init__(self, data, field_name, mode, context, config):
        self.data = data
        self.field_name = field_name
        self.mode = mode
        self.context = context
        self.config = config

    def __repr__(self):
        return "ValidationInfo(config=%r, context=%r, data=%r, field_name=%r)" % (self.config, self.context, self.data, self.field_name)


def _nparams(f, dflt=2):
    n = getattr(f, "__bv_np__", None)
    if n is None:
        code = getattr(f, "__code__", None)
        n = code.co_argcount if code is not None else dflt
    return n


class _FuncV:
    # AfterValidator and friends
    def __init__(self, func):
        self.func = func

    def __repr__(self):
        return "%s(func=%r)" % (type(self).__name__, self.func)


class AfterValidator(_FuncV):
    pass


class BeforeValidator(_FuncV):
    def __init__(self, func, json_schema_input_type=None):
        self.func = func


class PlainValidator(_FuncV):
    def __init__(self, func, json_schema_input_type=None):
        self.func = func


class WrapValidator(_FuncV):
    def __init__(self, func, json_schema_input_type=None):
        self.func = func


class _Fn:
    # a validator of Annotated metadata around the type's own
    def name(self):
        mv = self.mv
        fn = getattr(mv.func, "__name__", "<lambda>")
        if isinstance(mv, AfterValidator):
            return "function-after[%s(), %s]" % (fn, self.inner.name())
        if isinstance(mv, BeforeValidator):
            return "function-before[%s(), %s]" % (fn, self.inner.name())
        if isinstance(mv, PlainValidator):
            return "function-plain[%s()]" % fn
        return "function-wrap[%s()]" % fn

    def __init__(self, inner, mv):
        self.inner = inner
        self.mv = mv
        self.np = _nparams(mv.func, 1)

    def v(self, x, st):
        mv = self.mv
        info = ValidationInfo(None, None, st.mode, st.context, None)
        if isinstance(mv, AfterValidator):
            r = self.inner.v(x, st)
            return _call_user(mv.func, None, self.np, None, r, info, r)
        if isinstance(mv, BeforeValidator):
            y = _call_user(mv.func, None, self.np, None, x, info, x)
            return self.inner.v(y, st)
        if isinstance(mv, PlainValidator):
            return _call_user(mv.func, None, self.np, None, x, info, x)
        inner = self.inner
        return _call_user(mv.func, None, self.np - 1, None, x, info, x, _Handler(lambda y: inner.v(y, st)))

    def is_(self, v):
        return True if isinstance(self.mv, PlainValidator) else self.inner.is_(v)

    def ser(self, v, ctx):
        if isinstance(self.mv, PlainValidator):
            return _infer(v, ctx)
        return _ser(self.inner, v, ctx)


# ---------------------------------------------------------------- the state of one validation


class _St:
    def __init__(self, strict=False, mode="python", context=None, data=None, cfg=None):
        self.strict = strict
        self.mode = mode
        self.context = context
        self.ex = 2  # exactness: 2 exact, 1 strict, 0 lax
        self.fields_set = 0
        self.cfg = cfg


# ---------------------------------------------------------------- annotations


def _show(d):
    # an annotation as pydantic's FieldInfo repr shows it
    t = d[0]
    if t in ("int", "float", "str", "bool", "bytes"):
        return t
    if t == "none":
        return "NoneType"
    if t == "any":
        return d[1]
    if t in ("list", "set", "frozenset", "tuplevar"):
        if d[3]:
            return d[2]
        if t == "tuplevar":
            return "%s[%s, ...]" % (d[2], _show(d[1]))
        return "%s[%s]" % (d[2], _show(d[1]))
    if t == "dict":
        if d[4]:
            return d[3]
        return "%s[%s, %s]" % (d[3], _show(d[1]), _show(d[2]))
    if t == "tuple":
        if not d[1]:
            return d[2] + "[()]"
        return "%s[%s]" % (d[2], ", ".join(_show(x) for x in d[1]))
    if t == "union":
        return "Union[" + ", ".join(_show(x) for x in d[1]) + "]"
    if t == "literal":
        return "Literal[" + ", ".join(repr(x) for x in d[1]) + "]"
    if t == "ann":
        return "Annotated[%s, %s]" % (_show(d[1]), ", ".join(_show_meta(m) for m in d[2]))
    if t == "cls":
        v = d[1]
        if isinstance(v, tuple):
            return _show(v)
        return getattr(v, "__name__", repr(v))
    if t == "lazy":
        try:
            return _show(("cls", d[1]()))
        except NameError:
            return "ForwardRef(%r)" % d[2]
    return repr(d)


def _show_meta(m):
    # pydantic's display_as_type: its own Representation classes by repr, anything else by class name
    if isinstance(m, (FieldInfo, _StrictMark)):
        return repr(m)
    if isinstance(m, _Meta):
        return m.name
    return type(m).__name__


class _Ctx:
    # what building a validator needs: the model's config
    def __init__(self, cfg):
        self.cfg = cfg


def _build(d, cx, meta=None):
    # the validator for descriptor d, with the constraints in meta (FieldInfo metadata, Annotated extras)
    t = d[0]
    cons = {}
    strict = None
    fns = [m for m in meta if isinstance(m, _FuncV)] if meta else []
    if fns:
        v = _build(d, cx, [m for m in meta if not isinstance(m, _FuncV)])
        for m in fns:
            v = _Fn(v, m)
        return v
    if meta:
        for m in meta:
            if isinstance(m, _Meta):
                cons[m.key] = m.value
            elif isinstance(m, _General):
                for k, v in m.items:
                    if k == "strict":
                        strict = v
                    else:
                        cons[k] = v
            elif isinstance(m, FieldInfo):
                for mm in m.metadata:
                    if isinstance(mm, _Meta):
                        cons[mm.key] = mm.value
                    elif isinstance(mm, _General):
                        for k, v in mm.items:
                            if k == "strict":
                                strict = v
                            else:
                                cons[k] = v
            elif isinstance(m, _StrictMark):
                strict = m.strict
    if t == "ann":
        extra = list(d[2])
        if meta:
            extra = extra + list(meta)
        return _build(d[1], cx, extra)
    if t == "cls":
        v = d[1]
        if isinstance(v, tuple):
            return _build(v, cx, meta)
        return _build_cls(v, cx, cons, strict)
    if t == "lazy":
        return _Lazy(d[1], cx, cons, strict)
    if t == "int":
        return _Int(cons, strict)
    if t == "float":
        return _Float(cons, strict, cx)
    if t == "str":
        return _Str(cons, strict, cx)
    if t == "bool":
        return _Bool(strict)
    if t == "bytes":
        return _Bytes(cons, strict)
    if t == "none":
        return _None()
    if t == "any":
        return _Any()
    if t == "list":
        return _List(_build(d[1], cx), cons, strict, "List")
    if t == "set":
        return _Set(_build(d[1], cx), cons, strict, False)
    if t == "frozenset":
        return _Set(_build(d[1], cx), cons, strict, True)
    if t == "tuplevar":
        return _TupleVar(_build(d[1], cx), cons, strict)
    if t == "tuple":
        return _Tuple([_build(x, cx) for x in d[1]], cons, strict)
    if t == "dict":
        return _Dict(_build(d[1], cx), _build(d[2], cx), cons, strict)
    if t == "union":
        ch = [x for x in d[1] if x[0] != "none"]
        if len(ch) < len(d[1]):
            inner = _build(ch[0], cx) if len(ch) == 1 else _Union([_build(x, cx) for x in ch])
            return _Nullable(inner)
        return _Union([_build(x, cx) for x in ch])
    if t == "literal":
        return _Literal(d[1])
    raise PydanticSchemaGenerationError("Unable to generate pydantic-core schema for %r" % (d,))


class _StrictMark:
    def __init__(self, strict=True):
        self.strict = strict

    def __repr__(self):
        return "Strict(strict=%r)" % (self.strict,)


def Strict(strict=True):
    return _StrictMark(strict)


def _build_cls(c, cx, cons, strict):
    if isinstance(c, type):
        if c is int:
            return _Int(cons, strict)
        if c is float:
            return _Float(cons, strict, cx)
        if c is str:
            return _Str(cons, strict, cx)
        if c is bool:
            return _Bool(strict)
        if c is bytes:
            return _Bytes(cons, strict)
        if c is type(None):
            return _None()
        if c is object:
            return _Any()
        if issubclass(c, BaseModel):
            return _Model(c, strict)
        if _is_enum(c):
            return _Enum(c, strict, cx)
        if cx.cfg.get("arbitrary_types_allowed", False):
            return _IsInst(c)
    raise PydanticSchemaGenerationError(
        "Unable to generate pydantic-core schema for %r. Set `arbitrary_types_allowed=True` in the model_config to ignore this error or implement `__get_pydantic_core_schema__` on your type to fully support it.\n\nIf you got this error by calling handler(<some type>) with a custom type, check for a pydantic-core schema for <some type>, in particular for any type that does not have a `__get_pydantic_core_schema__` method.\n\nFor further information visit https://errors.pydantic.dev/2.13/u/schema-for-unknown-type" % (c,))


def _is_enum(c):
    for b in c.__mro__:
        if b.__name__ == "Enum" and b.__module__ == "enum":
            return True
    return False


# ---------------------------------------------------------------- validators
# Each has v(x, st): the value, or _Err; ser(v, mode): the value dumped; and is_(v): whether v is of its type
# (which member of a union dumps a value).


def _num_checks(x, cons, inp):
    if not cons:
        return x
    if "multiple_of" in cons:
        m = cons["multiple_of"]
        if isinstance(x, int) and isinstance(m, int):
            if x % m != 0:
                _fail("multiple_of", inp, {"multiple_of": m})
        elif x == x and x not in (float("inf"), float("-inf")):
            r = _math.fmod(x, m)
            th = abs(x) / 1e9
            if abs(r) > th and abs(r - m) > th:
                _fail("multiple_of", inp, {"multiple_of": m})
    if "le" in cons and not x <= cons["le"]:
        _fail("less_than_equal", inp, {"le": cons["le"]})
    if "lt" in cons and not x < cons["lt"]:
        _fail("less_than", inp, {"lt": cons["lt"]})
    if "ge" in cons and not x >= cons["ge"]:
        _fail("greater_than_equal", inp, {"ge": cons["ge"]})
    if "gt" in cons and not x > cons["gt"]:
        _fail("greater_than", inp, {"gt": cons["gt"]})
    return x


_INT_RE = _re.compile(r"[+-]?(?:0|[1-9](?:_?[0-9])*)?")


def _str_int(s):
    # pydantic-core's str -> int: whitespace around, a sign, `_` between digits, `.0` after (else int_from_float)
    t = s.strip()
    if t.startswith("+") or t.startswith("-"):
        sign, body = t[0], t[1:]
    else:
        sign, body = "", t
    if "." in body:
        whole, frac = body.split(".", 1)
        if whole and _digits(whole) and frac and frac.isdigit() and frac.isascii():
            if frac.strip("0") == "":
                body = whole
            else:
                return None, "int_parsing"
        elif whole and _digits(whole) and frac == "":
            body = whole
        else:
            return None, "int_parsing"
    if not body or not _digits(body):
        return None, "int_parsing"
    body = body.replace("_", "")
    if len(body) > 4300:
        return None, "int_parsing_size"
    v = int(body)
    return (-v if sign == "-" else v), None


def _text(x):
    if type(x) is bytes:
        try:
            return x.decode("utf-8")
        except UnicodeDecodeError:
            return "\x00"
    return x


def _digits(b):
    if not b.isascii():
        return False
    if b[0] == "_" or b[-1] == "_" or "__" in b:
        return False
    return b.replace("_", "").isdigit()


class _Int:
    def name(self):
        return "constrained-int" if self.cons else "int"

    def __init__(self, cons, strict):
        self.cons = cons
        self.strict = strict

    def v(self, x, st):
        strict = st.strict if self.strict is None else self.strict
        tx = type(x)
        if tx is int:
            r = x
        elif tx is bool:
            if strict:
                _fail("int_type", x)
            st.ex = min(st.ex, 0)
            r = int(x)
        elif isinstance(x, int):
            st.ex = min(st.ex, 1)
            r = int(x)
        elif tx is float:
            if strict:
                _fail("int_type", x)
            st.ex = min(st.ex, 0)
            if x != x or x in (float("inf"), float("-inf")):
                _fail("finite_number", x)
            if x % 1 != 0:
                _fail("int_from_float", x)
            r = int(x)
        elif isinstance(x, str) or tx is bytes:
            if strict:
                _fail("int_type", x)
            st.ex = min(st.ex, 0)
            r, e = _str_int(_text(x))
            if e is not None:
                _fail(e, x)
        elif not strict and _is_enum(tx):
            # pydantic-core takes the member's value as it is (and a constrained int reads it as text)
            st.ex = min(st.ex, 0)
            if self.cons:
                _fail("int_parsing_size", x.value)
            return x.value
        else:
            _fail("int_type", x)
        if self.cons:
            _num_checks(r, self.cons, x)
        return r

    def is_(self, v):
        return isinstance(v, int) and not isinstance(v, bool)

    def ser(self, v, ctx):
        return v


_FLOAT_RE = _re.compile(r"[+-]?(?:[0-9](?:_?[0-9])*(?:\.(?:[0-9](?:_?[0-9])*)?)?|\.[0-9](?:_?[0-9])*)(?:[eE][+-]?[0-9]+)?")


def _str_float(s):
    t = s.strip()
    low = t.lower()
    if low in ("inf", "+inf", "infinity", "+infinity"):
        return float("inf")
    if low in ("-inf", "-infinity"):
        return float("-inf")
    if low in ("nan", "+nan", "-nan"):
        return float("nan")
    if not t.isascii() or _FLOAT_RE.fullmatch(t) is None:
        return None
    return float(t.replace("_", ""))


class _Float:
    def name(self):
        return "constrained-float" if self.cons else "float"

    def __init__(self, cons, strict, cx):
        if cons:
            cons = {k: (float(v) if (k != "allow_inf_nan" and isinstance(v, int) and not isinstance(v, bool)) else v) for k, v in cons.items()}
        self.cons = cons
        self.strict = strict
        self.inf_nan = cons.pop("allow_inf_nan", True) if cons else True

    def v(self, x, st):
        strict = st.strict if self.strict is None else self.strict
        tx = type(x)
        if tx is float:
            r = x
        elif tx is int:
            st.ex = min(st.ex, 1)
            r = float(x)
        elif tx is bool:
            if strict:
                _fail("float_type", x)
            st.ex = min(st.ex, 0)
            r = float(x)
        elif isinstance(x, float):
            st.ex = min(st.ex, 1)
            r = float(x)
        elif isinstance(x, int):
            st.ex = min(st.ex, 1)
            r = float(x)
        elif isinstance(x, str) or tx is bytes:
            if strict:
                _fail("float_type", x)
            st.ex = min(st.ex, 0)
            r = _str_float(_text(x))
            if r is None:
                _fail("float_parsing", x)
        else:
            _fail("float_type", x)
        if not self.inf_nan and (r != r or r in (float("inf"), float("-inf"))):
            _fail("finite_number", x)
        if self.cons:
            _num_checks(r, self.cons, x)
        return r

    def is_(self, v):
        return isinstance(v, float) or (isinstance(v, int) and not isinstance(v, bool))

    def ser(self, v, ctx):
        if ctx.json and type(v) is int:
            return float(v)
        return v


class _Str:
    def name(self):
        c = self.min is not None or self.max is not None or self.rx is not None or self.strip or self.lower or self.upper
        return "constrained-str" if c else "str"

    def __init__(self, cons, strict, cx):
        cfg = cx.cfg
        self.strict = strict
        self.strip = cfg.get("str_strip_whitespace", False)
        self.lower = cfg.get("str_to_lower", False)
        self.upper = cfg.get("str_to_upper", False)
        self.min = cons.get("min_length", cfg.get("str_min_length"))
        self.max = cons.get("max_length", cfg.get("str_max_length"))
        p = cons.get("pattern")
        self.pattern = p
        self.rx = _re.compile(p) if p is not None else None

    def v(self, x, st):
        strict = st.strict if self.strict is None else self.strict
        tx = type(x)
        if tx is str:
            r = x
        elif isinstance(x, str):
            if _is_enum(type(x)):
                st.ex = min(st.ex, 1)
                r = x.value
            else:
                st.ex = min(st.ex, 1)
                r = str(x)
        elif not strict and _is_enum(tx):
            st.ex = min(st.ex, 0)
            r = str(x.value)
        elif tx is bytes:
            if strict:
                _fail("string_type", x)
            st.ex = min(st.ex, 0)
            try:
                r = x.decode("utf-8")
            except UnicodeDecodeError:
                _fail("string_unicode", x)
        else:
            _fail("string_type", x)
        if self.strip:
            r = r.strip()
        if self.min is not None and len(r) < self.min:
            _fail("string_too_short", x, {"min_length": self.min})
        if self.max is not None and len(r) > self.max:
            _fail("string_too_long", x, {"max_length": self.max})
        if self.lower:
            r = r.lower()
        if self.upper:
            r = r.upper()
        if self.rx is not None and self.rx.search(r) is None:
            _fail("string_pattern_mismatch", x, {"pattern": self.pattern})
        return r

    def is_(self, v):
        return isinstance(v, str)

    def ser(self, v, ctx):
        return v


_TRUE = ("1", "on", "t", "true", "y", "yes")
_FALSE = ("0", "off", "f", "false", "n", "no")


class _Bool:
    def name(self):
        return "bool"

    def __init__(self, strict):
        self.strict = strict

    def v(self, x, st):
        strict = st.strict if self.strict is None else self.strict
        if x is True or x is False:
            return x
        if strict:
            _fail("bool_type", x)
        if isinstance(x, str) or type(x) is bytes:
            st.ex = min(st.ex, 0)
            s = _text(x).lower()
            if s in _TRUE:
                return True
            if s in _FALSE:
                return False
            _fail("bool_parsing", x)
        if isinstance(x, int):
            st.ex = min(st.ex, 0)
            if x == 1:
                return True
            if x == 0:
                return False
            _fail("bool_parsing" if -(2 ** 63) <= x < 2 ** 63 else "bool_type", x)
        if isinstance(x, float):
            st.ex = min(st.ex, 0)
            if x == 1.0:
                return True
            if x == 0.0:
                return False
            _fail("bool_parsing" if x % 1 == 0 else "bool_type", x)
        _fail("bool_type", x)

    def is_(self, v):
        return isinstance(v, bool)

    def ser(self, v, ctx):
        return v


class _Bytes:
    def name(self):
        return "constrained-bytes" if (self.min is not None or self.max is not None) else "bytes"

    def __init__(self, cons, strict):
        self.strict = strict
        self.min = cons.get("min_length")
        self.max = cons.get("max_length")

    def v(self, x, st):
        strict = st.strict if self.strict is None else self.strict
        if type(x) is bytes:
            r = x
        elif isinstance(x, str) and (not strict or st.mode == "json"):
            st.ex = min(st.ex, 0)
            r = x.encode("utf-8")
        else:
            _fail("bytes_type", x)
        if self.min is not None and len(r) < self.min:
            _fail("bytes_too_short", r, {"min_length": self.min})
        if self.max is not None and len(r) > self.max:
            _fail("bytes_too_long", r, {"max_length": self.max})
        return r

    def is_(self, v):
        return isinstance(v, bytes)

    def ser(self, v, ctx):
        if ctx.json:
            return v.decode("utf-8")
        return v


class _None:
    def name(self):
        return "none"

    def v(self, x, st):
        if x is None:
            return x
        _fail("none_required", x)

    def is_(self, v):
        return v is None

    def ser(self, v, ctx):
        return v


class _Any:
    def name(self):
        return "any"

    def v(self, x, st):
        return x

    def is_(self, v):
        return True

    def ser(self, v, ctx):
        return _infer(v, ctx)


class _Nullable:
    def name(self):
        return "nullable[%s]" % self.inner.name()

    def __init__(self, inner):
        self.inner = inner

    def v(self, x, st):
        if x is None:
            return None
        return self.inner.v(x, st)

    def is_(self, v):
        return v is None or self.inner.is_(v)

    def ser(self, v, ctx):
        if v is None:
            return None
        return _ser(self.inner, v, ctx)


def _len_checks(r, cons, kind, x):
    if not cons:
        return
    n = len(r)
    mn = cons.get("min_length")
    if mn is not None and n < mn:
        _fail("too_short", x, {"field_type": kind, "min_length": mn, "actual_length": n})
    mx = cons.get("max_length")
    if mx is not None and n > mx:
        _fail("too_long", x, {"field_type": kind, "max_length": mx, "actual_length": n})


def _seq_input(x, st, strict, exact_type, err):
    # the items of a list, tuple or set input, as pydantic-core takes them in lax mode
    tx = type(x)
    if tx is exact_type:
        return x
    if st.mode == "json" and tx is list:
        return x
    if strict:
        _fail(err, x)
    if isinstance(x, (str, bytes, dict)):
        _fail(err, x)
    if isinstance(x, BaseModel):
        st.ex = min(st.ex, 0)
        return list(x)
    if isinstance(x, (list, tuple, set, frozenset)):
        st.ex = min(st.ex, 0 if not isinstance(x, exact_type) else 1)
        return x
    tn = tx.__name__
    if tn in ("deque", "dict_keys", "dict_values", "generator", "map", "filter", "range", "zip", "enumerate", "reversed", "list_iterator"):
        st.ex = min(st.ex, 0)
        return list(x)
    _fail(err, x)


class _List:
    def name(self):
        return "list[%s]" % self.item.name()

    def __init__(self, item, cons, strict, kind):
        self.item = item
        self.cons = cons
        self.strict = strict
        self.kind = kind

    def v(self, x, st):
        strict = st.strict if self.strict is None else self.strict
        items = _seq_input(x, st, strict, list, "list_type")
        out = []
        errs = []
        it = self.item
        if isinstance(it, _Any):
            out = list(items)
        else:
            i = 0
            for e in items:
                try:
                    out.append(it.v(e, st))
                except _Err as er:
                    errs.extend(_at(er.lines, i))
                i += 1
        if errs:
            raise _Err(errs)
        _len_checks(out, self.cons, self.kind, x)
        return out

    def is_(self, v):
        return isinstance(v, list)

    def ser(self, v, ctx):
        it = self.item
        return [_ser(it, e, ctx) for e in v]


class _Set:
    def name(self):
        return ("frozenset[%s]" if self.frozen else "set[%s]") % self.item.name()

    def __init__(self, item, cons, strict, frozen):
        self.item = item
        self.cons = cons
        self.strict = strict
        self.frozen = frozen

    def v(self, x, st):
        strict = st.strict if self.strict is None else self.strict
        want = frozenset if self.frozen else set
        err = "frozen_set_type" if self.frozen else "set_type"
        tx = type(x)
        if tx is want:
            items = x
        elif st.mode == "json" and tx is list:
            items = x
        elif strict:
            _fail(err, x)
        elif isinstance(x, (str, bytes, dict)):
            _fail(err, x)
        elif isinstance(x, (list, tuple, set, frozenset)):
            st.ex = min(st.ex, 0)
            items = x
        else:
            items = _seq_input(x, st, strict, want, err)
        out = []
        errs = []
        i = 0
        for e in items:
            try:
                out.append(self.item.v(e, st))
            except _Err as er:
                errs.extend(_at(er.lines, i))
            i += 1
        if errs:
            raise _Err(errs)
        r = want(out)
        _len_checks(r, self.cons, "Frozenset" if self.frozen else "Set", x)
        return r

    def is_(self, v):
        return isinstance(v, frozenset if self.frozen else set)

    def ser(self, v, ctx):
        if ctx.json:
            return [_ser(self.item, e, ctx) for e in v]
        out = [_ser(self.item, e, ctx) for e in v]
        return frozenset(out) if self.frozen else set(out)


class _TupleVar:
    def name(self):
        return "tuple[%s, ...]" % self.item.name()

    def __init__(self, item, cons, strict):
        self.item = item
        self.cons = cons
        self.strict = strict

    def v(self, x, st):
        strict = st.strict if self.strict is None else self.strict
        items = _seq_input(x, st, strict, tuple, "tuple_type")
        out = []
        errs = []
        i = 0
        for e in items:
            try:
                out.append(self.item.v(e, st))
            except _Err as er:
                errs.extend(_at(er.lines, i))
            i += 1
        if errs:
            raise _Err(errs)
        _len_checks(out, self.cons, "Tuple", x)
        return tuple(out)

    def is_(self, v):
        return isinstance(v, tuple)

    def ser(self, v, ctx):
        out = [_ser(self.item, e, ctx) for e in v]
        return out if ctx.json else tuple(out)


class _Tuple:
    def name(self):
        return "tuple[%s]" % ", ".join(i.name() for i in self.items)

    def __init__(self, items, cons, strict):
        self.items = items
        self.cons = cons
        self.strict = strict

    def v(self, x, st):
        strict = st.strict if self.strict is None else self.strict
        items = list(_seq_input(x, st, strict, tuple, "tuple_type"))
        n = len(self.items)
        out = []
        errs = []
        for i in range(n):
            if i < len(items):
                try:
                    out.append(self.items[i].v(items[i], st))
                except _Err as er:
                    errs.extend(_at(er.lines, i))
            else:
                ln = _Line("missing", x)
                ln.loc.append(i)
                errs.append(ln)
        if len(items) > n:
            errs.append(_Line("too_long", x, {"field_type": "Tuple", "max_length": n, "actual_length": len(items)}))
        if errs:
            raise _Err(errs)
        return tuple(out)

    def is_(self, v):
        return isinstance(v, tuple)

    def ser(self, v, ctx):
        out = []
        for i in range(len(v)):
            out.append(_ser(self.items[i], v[i], ctx) if i < len(self.items) else _infer(v[i], ctx))
        return out if ctx.json else tuple(out)


class _Dict:
    def name(self):
        return "dict[%s,%s]" % (self.k.name(), self.val.name())

    def __init__(self, k, val, cons, strict):
        self.k = k
        self.val = val
        self.cons = cons
        self.strict = strict

    def v(self, x, st):
        strict = st.strict if self.strict is None else self.strict
        if type(x) is dict:
            items = x
        elif isinstance(x, dict) and not strict:
            st.ex = min(st.ex, 1)
            items = x
        else:
            _fail("dict_type", x)
        out = {}
        errs = []
        kv = self.k
        vv = self.val
        for key, value in items.items():
            ok = True
            try:
                k2 = kv.v(key, st)
            except _Err as er:
                for ln in er.lines:
                    ln.loc[0:0] = [key, "[key]"]
                errs.extend(er.lines)
                ok = False
            try:
                v2 = vv.v(value, st)
            except _Err as er:
                errs.extend(_at(er.lines, key))
                ok = False
            if ok:
                out[k2] = v2
        if errs:
            raise _Err(errs)
        _len_checks(out, self.cons, "Dictionary", x)
        return out

    def is_(self, v):
        return isinstance(v, dict)

    def ser(self, v, ctx):
        out = {}
        for k, e in v.items():
            k2 = _ser(self.k, k, ctx)
            if ctx.json:
                k2 = _json_key(k2)
            out[k2] = _ser(self.val, e, ctx)
        return out


def _json_key(k):
    if isinstance(k, str):
        return k
    if k is True:
        return "true"
    if k is False:
        return "false"
    if k is None:
        return "None"
    if isinstance(k, float):
        return _jfloat(k)
    if _is_enum(type(k)):
        return _json_key(k.value)
    return str(k)


def _expected(vals):
    r = [repr(v) for v in vals]
    if len(r) == 1:
        return r[0]
    return ", ".join(r[:-1]) + " or " + r[-1]


class _Literal:
    def name(self):
        return "literal[%s]" % ",".join(repr(v) for v in self.vals)

    def __init__(self, vals):
        self.vals = list(vals)
        self.expected = _expected(self.vals)

    def v(self, x, st):
        for e in self.vals:
            if type(e) is type(x) and e == x:
                return e
        for e in self.vals:
            # ints and bools (and int enum members) match by equality, as in pydantic-core's dict of values
            if isinstance(x, int) and isinstance(e, int) and not _is_enum(type(e)) and x == e:
                return e
        if isinstance(x, int) and not isinstance(x, bool) and not (-(2 ** 63) <= x < 2 ** 63) and any(type(e) is int for e in self.vals):
            _fail("int_parsing_size", x)
        if not st.strict:
            # lax, as pydantic-core: a bool or a whole float for an int, a str subclass for a str, an enum's value
            for e in self.vals:
                te = type(e)
                if te is bool:
                    if (isinstance(x, int) and not isinstance(x, bool) or type(x) is float) and x in (0, 1) and bool(x) is e:
                        return e
                elif te is int:
                    if x is True or x is False:
                        if int(x) == e:
                            return e
                    elif isinstance(x, int) and int(x) == e:
                        return e
                    elif type(x) is float and x == e:
                        return e
                elif te is str:
                    if isinstance(x, str) and str(x) == e:
                        return e
                elif _is_enum(te):
                    if x == e or (type(e.value) is type(x) and e.value == x):
                        return e
        _fail("literal_error", x, {"expected": self.expected})

    def is_(self, v):
        return any(v == e for e in self.vals)

    def ser(self, v, ctx):
        return _infer(v, ctx)


class _Enum:
    def name(self):
        pre = {int: "int-", str: "str-", float: "float-"}.get(self.base, "")
        n = "%senum[%s]" % (pre, self.cls.__name__)
        if self.use_values:
            return "function-after[operator.attrgetter('value')(), %s]" % n
        return n

    def __init__(self, cls, strict, cx):
        self.cls = cls
        self.strict = strict
        self.members = list(cls)
        self.expected = _expected([m.value for m in self.members])
        self.use_values = cx.cfg.get("use_enum_values", False)
        base = None
        for b in cls.__mro__:
            if b is int or b is str or b is float:
                base = b
                break
        self.base = base

    def _out(self, m):
        return m.value if self.use_values else m

    def v(self, x, st):
        strict = st.strict if self.strict is None else self.strict
        if type(x) is self.cls:
            return self._out(x)
        if isinstance(x, self.cls):
            st.ex = min(st.ex, 1)
            return self._out(x)
        if strict and st.mode == "python":
            _fail("is_instance_of", x, {"class": self.cls.__name__})
        st.ex = min(st.ex, 0)
        for m in self.members:
            mv = m.value
            if type(mv) is type(x) and mv == x:
                return self._out(m)
        if st.mode == "json" and strict:
            _fail("enum", x, {"expected": self.expected})
        if self.base is int and isinstance(x, int) and not isinstance(x, bool) and not (-(2 ** 63) <= x < 2 ** 63):
            _fail("int_parsing_size", x)
        if self.base is int and _is_enum(type(x)) and not isinstance(x, int) and not isinstance(x.value, int):
            # pydantic-core takes the other enum's value as an int's text
            _fail("int_parsing_size", x.value)
        if self.base is int and (x is True or x is False or type(x) is bytes):
            try:
                r = _Int({}, False).v(x, st)
            except _Err:
                r = None
            for m in self.members:
                if r is not None and m.value == r:
                    return self._out(m)
        if self.base is int or self.base is None:
            if isinstance(x, str) and not isinstance(x, bool):
                r, err = _str_int(x)
                if err is None:
                    for m in self.members:
                        if type(m.value) is int and m.value == r:
                            return self._out(m)
            if isinstance(x, float) and x % 1 == 0:
                for m in self.members:
                    if type(m.value) is int and m.value == x:
                        return self._out(m)
        if self.base is float and isinstance(x, (int, str)) and not isinstance(x, bool):
            try:
                f = float(x)
            except ValueError:
                f = None
            for m in self.members:
                if m.value == f:
                    return self._out(m)
        _fail("enum", x, {"expected": self.expected})

    def is_(self, v):
        return isinstance(v, self.cls)

    def ser(self, v, ctx):
        if ctx.json and _is_enum(type(v)):
            return _infer(v.value, ctx)
        return v


class _IsInst:
    # a class of the program's own under arbitrary_types_allowed: isinstance, nothing more
    def __init__(self, cls):
        self.cls = cls

    def name(self):
        return "is-instance[%s]" % self.cls.__name__

    def v(self, x, st):
        if st.mode == "json":
            _fail("needs_python_object", x, {"method_name": "isinstance"})
        if isinstance(x, self.cls):
            return x
        _fail("is_instance_of", x, {"class": self.cls.__name__})

    def is_(self, v):
        return isinstance(v, self.cls)

    def ser(self, v, ctx):
        return _infer(v, ctx)


class _Union:
    def __init__(self, choices):
        self.choices = choices
        self.labels = None

    def name(self):
        return "union[%s]" % ",".join(c.name() for c in self.choices)

    def v(self, x, st):
        # pydantic-core's smart mode: an exact match wins at once; else the most exact success, more fields
        # set breaking a tie; else every member's errors
        best = None
        best_key = None
        errs = []
        if self.labels is None:
            self.labels = [c.name() for c in self.choices]
        ex0 = st.ex
        fs0 = st.fields_set
        for i in range(len(self.choices)):
            st.ex = 2
            st.fields_set = 0
            try:
                r = self.choices[i].v(x, st)
            except _Err as er:
                if best is None:
                    errs.extend(_at(er.lines, self.labels[i]))
                continue
            if st.ex == 2 and not isinstance(self.choices[i], _Model):
                st.ex = min(ex0, 2)
                st.fields_set = fs0
                return r
            key = (st.ex, st.fields_set)
            if best is None or key > best_key:
                best = (r,)
                best_key = key
        if best is not None:
            st.ex = min(ex0, best_key[0])
            st.fields_set = fs0
            return best[0]
        st.ex = ex0
        st.fields_set = fs0
        raise _Err(errs)

    def is_(self, v):
        return any(c.is_(v) for c in self.choices)

    def ser(self, v, ctx):
        for c in self.choices:
            if c.is_(v) and _exact_is(c, v):
                return c.ser(v, ctx)
        for c in self.choices:
            if c.is_(v):
                return c.ser(v, ctx)
        return _infer(v, ctx)


def _exact_is(c, v):
    if isinstance(c, _Int):
        return type(v) is int
    if isinstance(c, _Float):
        return type(v) is float
    if isinstance(c, _Model):
        return type(v) is c.cls
    return True


class _Lazy:
    # a forward reference: built when first used
    def name(self):
        return self._get().name()

    def __init__(self, thunk, cx, cons, strict):
        self.thunk = thunk
        self.cx = cx
        self.cons = cons
        self.strict = strict
        self.real = None

    def _get(self):
        if self.real is None:
            c = self.thunk()
            if isinstance(c, tuple):
                self.real = _build(c, self.cx)
            else:
                self.real = _build_cls(c, self.cx, self.cons, self.strict)
        return self.real

    def v(self, x, st):
        return self._get().v(x, st)

    def is_(self, v):
        return self._get().is_(v)

    def ser(self, v, ctx):
        return self._get().ser(v, ctx)


class _Model:
    def name(self):
        return self.cls.__name__

    def __init__(self, cls, strict):
        self.cls = cls
        self.strict = strict

    def v(self, x, st):
        cls = self.cls
        if isinstance(x, cls):
            if type(x) is not cls:
                st.ex = min(st.ex, 1)
            if _cfg(cls).get("revalidate_instances", "never") == "always":
                return _validate_model(cls, _obj_dict(x), st, None)
            st.fields_set = st.fields_set + len(x.__pydantic_fields_set__)
            return x
        st.ex = min(st.ex, 1)
        r = _validate_model(cls, x, st, None)
        return r

    def is_(self, v):
        return isinstance(v, self.cls)

    def ser(self, v, ctx):
        if not isinstance(v, BaseModel):
            return _infer(v, ctx)
        return _dump_model(v, ctx, self.cls if isinstance(v, self.cls) else type(v))


# ---------------------------------------------------------------- models


def _copy_default(v):
    t = type(v)
    if t in (int, float, str, bool, bytes, type(None), tuple, frozenset) or v is PydanticUndefined:
        return v
    if t is list:
        return [_copy_default(e) for e in v]
    if t is dict:
        return {k: _copy_default(e) for k, e in v.items()}
    if t is set:
        return set(v)
    if isinstance(v, BaseModel):
        return v.model_copy(deep=True)
    return v


class _Fld:
    # a field of a model, ready to validate
    def __init__(self, name, info, desc):
        self.name = name
        self.info = info
        self.desc = desc
        self.val = None
        # the field's decorator validators in the order the class defines them; each wraps the ones before it
        self.chain = []
        self.alias = info.validation_alias
        self.ser_alias = info.serialization_alias


class _Schema:
    def __init__(self):
        self.fields = []
        self.by_name = {}
        self.before = []
        self.after = []
        self.wrap = []
        self.computed = []
        self.built = False


class _ClsInfo:
    # what this copy keeps of a model class (pydantic keeps it in __pydantic_fields__ and the like)
    def __init__(self):
        self.cfg = {}
        self.fields = {}
        self.descs = {}
        self.order = []
        self.decs = {}
        self.schema = None
        # a class of the program defines model_post_init
        self.post_init = False


_INFO = {}


def _I(cls):
    i = _INFO.get(cls)
    if i is None:
        i = _ClsInfo()
        _INFO[cls] = i
    return i


def _has_lazy(d):
    if isinstance(d, tuple):
        if d and d[0] == "lazy":
            return True
        return any(_has_lazy(x) for x in d)
    return False


def _cfg(cls):
    return _I(cls).cfg


def _own_ann(cls):
    if _ann_hook is not None:
        return _ann_hook(cls)
    a = cls.__dict__.get("__annotations__")
    if a is None:
        return ()
    return a


_ann_hook = None


def _setup(cls):
    # what pydantic's metaclass does when a model class is made: its fields, config and decorators
    me = _I(cls)
    cfg = {}
    for b in reversed(cls.__mro__[1:]):
        if b in _INFO:
            cfg.update(_INFO[b].cfg)
    own = cls.__dict__.get("model_config")
    if own:
        cfg.update(own)
    me.cfg = cfg
    cls.model_config = dict(cfg)
    fields = {}
    descs = {}
    classvars = set()
    private = {}
    for b in reversed(cls.__mro__[1:]):
        if b is BaseModel or not issubclass(b, BaseModel):
            continue
        fields.update(_I(b).fields)
        descs.update(_I(b).descs)
        classvars.update(b.__dict__.get("__class_vars__", set()))
        private.update(b.__dict__.get("__private_attributes__", {}))
    mine = {}
    for name, d in _own_ann(cls):
        if d[0] == "classvar":
            classvars.add(name)
            continue
        if name.startswith("_"):
            dv = cls.__dict__.get(name, PydanticUndefined)
            private[name] = dv if isinstance(dv, ModelPrivateAttr) else ModelPrivateAttr(dv)
            continue
        dv = cls.__dict__.get(name, PydanticUndefined)
        if isinstance(dv, FieldInfo):
            info = dv
        else:
            info = FieldInfo(dv)
        # an alias (`Tags = list[str]`, PositiveInt) is what it stands for; Annotated's extras go to the field
        while d[0] == "cls" and isinstance(d[1], tuple):
            d = d[1]
        while d[0] == "ann":
            for m in d[2]:
                if isinstance(m, FieldInfo):
                    _merge_info(info, m)
                else:
                    info.metadata = info.metadata + [m]
            d = d[1]
            while d[0] == "cls" and isinstance(d[1], tuple):
                d = d[1]
        info._desc = d
        info.annotation = d
        mine[name] = info
        descs[name] = d
    for name, v in list(cls.__dict__.items()):
        if isinstance(v, ModelPrivateAttr) and name not in private:
            private[name] = v
    for name in mine:
        if name in fields:
            del fields[name]
        fields[name] = mine[name]
    # fields keep the order of the first class that has them
    order = {}
    for b in reversed(cls.__mro__):
        for name in (_INFO[b].order if b in _INFO else ()):
            if name not in order:
                order[name] = len(order)
    for name in mine:
        if name not in order:
            order[name] = len(order)
    names = sorted(fields, key=lambda n: order[n])
    me.fields = {n: fields[n] for n in names}
    me.descs = descs
    me.order = names
    me.post_init = any(b is not BaseModel and "model_post_init" in b.__dict__ for b in cls.__mro__)
    cls.__class_vars__ = classvars
    cls.__private_attributes__ = private
    # pydantic takes private defaults off the class, so `self._x` finds the instance's value
    for name in private:
        if name in cls.__dict__:
            delattr(cls, name)
    cls.model_fields = {n: fields[n] for n in names}
    decs = {}
    for b in reversed(cls.__mro__):
        if not issubclass(b, BaseModel) and b is not cls:
            continue
        for k, v in b.__dict__.items():
            if isinstance(v, _Dec):
                decs[k] = v
    me.decs = decs
    computed = {}
    for k, v in decs.items():
        if v.kind == "computed":
            computed[k] = v
            if cls.__dict__.get(k) is v:
                setattr(cls, k, v.f)
    cls.model_computed_fields = computed
    me.schema = None


def _merge_info(info, m):
    for k in ["default_factory", "alias", "validation_alias", "serialization_alias", "title", "description", "examples", "exclude", "frozen", "validate_default"]:
        if getattr(m, k) is not None and getattr(info, k) is None:
            setattr(info, k, getattr(m, k))
    if m.default is not PydanticUndefined and info.default is PydanticUndefined:
        info.default = m.default
    if not m.repr:
        info.repr = False
    if m.alias is not None:
        info.alias_priority = 2
    info.metadata = info.metadata + m.metadata


def _schema(cls):
    me = _I(cls)
    s = me.schema
    if s is not None:
        return s
    cfg = _cfg(cls)
    cx = _Ctx(cfg)
    s = _Schema()
    for name, info in me.fields.items():
        f = _Fld(name, info, info._desc)
        f.val = _build(info._desc, cx, info.metadata)
        s.fields.append(f)
        s.by_name[name] = f
    for k, dv in me.decs.items():
        if dv.kind == "field":
            fn, takes_cls = _unwrap(dv.f)
            targets = s.fields if "*" in dv.fields else [s.by_name[n] for n in dv.fields if n in s.by_name]
            for f in targets:
                f.chain.append((dv.mode, fn, takes_cls, _nparams(fn)))
        elif dv.kind == "model":
            fn, takes_cls = _unwrap(dv.f)
            item = (fn, takes_cls, _nparams(fn))
            if dv.mode == "before":
                s.before.insert(0, item)
            elif dv.mode == "after":
                s.after.append(item)
            elif dv.mode == "wrap":
                s.wrap.insert(0, item)
        elif dv.kind == "computed":
            s.computed.append((k, dv))
    s.built = True
    me.schema = s
    return s


def _call_user(fn, takes_cls, np, cls, v, info, inp, extra=None):
    # a validator the program wrote: ValueError and AssertionError become validation errors
    args = []
    if takes_cls is True:
        args.append(cls)
    args.append(v)
    if extra is not None:
        args.append(extra)
    want = np - (1 if takes_cls is True else 0)
    if want > len(args) - (1 if takes_cls is True else 0):
        args.append(info)
    try:
        return fn(*args)
    except _Err:
        raise
    except PydanticCustomError as e:
        raise _Err([_Line(e.type, inp, e.context, e.message(), True)])
    except ValidationError as e:
        raise _Err([_copy_line(ln) for ln in e._lines])
    except ValueError as e:
        raise _Err([_Line("value_error", inp, {"error": e})])
    except AssertionError as e:
        raise _Err([_Line("assertion_error", inp, {"error": e})])


def _copy_line(ln):
    n = _Line(ln.t, ln.inp, ln.ctx, ln.msg, ln.custom)
    n.loc = list(ln.loc)
    return n


def _run_field(cls, f, x, st, data):
    if not f.chain:
        # no field validators: just the type's validator (nothing would see the ValidationInfo)
        return f.val.v(x, st)
    info = ValidationInfo(data, f.name, st.mode, st.context, cls.model_config)

    def layer(k, y):
        # validator k around validators 0..k-1 around the type; an error carries the input of its own layer
        if k < 0:
            return f.val.v(y, st)
        mode, fn, tc, np = f.chain[k]
        if mode == "before":
            return layer(k - 1, _call_user(fn, tc, np, cls, y, info, y))
        if mode == "after":
            return _call_user(fn, tc, np, cls, layer(k - 1, y), info, y)
        if mode == "plain":
            return _call_user(fn, tc, np, cls, y, info, y)
        return _call_user(fn, tc, np - 1, cls, y, info, y, _Handler(lambda z: layer(k - 1, z)))

    return layer(len(f.chain) - 1, x)


class _Handler:
    def __init__(self, f):
        self.f = f

    def __call__(self, v, outer_location=None):
        return self.f(v)


def _obj_dict(x):
    d = {}
    for k in _I(type(x)).fields:
        d[k] = getattr(x, k)
    return d


def _validate_fields(cls, s, data, st):
    cfg = _cfg(cls)
    extra_mode = cfg.get("extra", "ignore")
    by_name = cfg.get("populate_by_name", False) or cfg.get("validate_by_name", False)
    by_alias = cfg.get("validate_by_alias", True)
    out = {}
    fset = set()
    errs = []
    used = set()
    for f in s.fields:
        key = f.name
        present = False
        if f.alias is not None and by_alias:
            if f.alias in data:
                key = f.alias
                present = True
            elif by_name and f.name in data:
                present = True
        elif f.name in data:
            present = True
        if present:
            used.add(key)
            try:
                out[f.name] = _run_field(cls, f, data[key], st, out)
                fset.add(f.name)
            except _Err as er:
                errs.extend(_at(er.lines, key))
            continue
        info = f.info
        if info.default_factory is not None:
            dv = info.default_factory()
        elif info.default is not PydanticUndefined:
            dv = _copy_default(info.default)
        else:
            ln = _Line("missing", data)
            ln.loc.append(f.alias if (f.alias is not None and by_alias) else f.name)
            errs.append(ln)
            continue
        if info.validate_default or cfg.get("validate_default", False):
            # a default is a Python value, even under model_validate_json
            mode = st.mode
            st.mode = "python"
            try:
                dv = _run_field(cls, f, dv, st, out)
            except _Err as er:
                errs.extend(_at(er.lines, f.name))
                continue
            finally:
                st.mode = mode
        out[f.name] = dv
    extra = None
    if extra_mode == "allow":
        extra = {}
    for k in data:
        if k in used:
            continue
        if extra_mode == "forbid":
            ln = _Line("extra_forbidden", data[k])
            ln.loc.append(k)
            errs.append(ln)
        elif extra_mode == "allow":
            if k not in s.by_name or (s.by_name[k].alias is not None and not by_name):
                extra[k] = data[k]
                fset.add(k)
    if errs:
        raise _Err(errs)
    st.fields_set = st.fields_set + len(fset)
    return out, fset, extra


def _validate_model(cls, data, st, inst):
    s = _schema(cls)
    if s.wrap:
        def chain(k):
            if k == len(s.wrap):
                return lambda d: _validate_model_inner(cls, s, d, st, inst)
            fn, tc, np = s.wrap[k]
            nxt = chain(k + 1)

            def h(d):
                info = ValidationInfo({}, None, st.mode, st.context, cls.model_config)
                return _call_user(fn, tc, np - 1, cls, d, info, d, _Handler(nxt))
            return h
        r = chain(0)(data)
        return r
    return _validate_model_inner(cls, s, data, st, inst)


def _validate_model_inner(cls, s, data, st, inst):
    info = ValidationInfo({}, None, st.mode, st.context, cls.model_config) if (s.before or s.after) else None
    for fn, tc, np in s.before:
        data = _call_user(fn, tc, np, cls, data, info, data)
    if isinstance(data, cls) and inst is None:
        if _cfg(cls).get("revalidate_instances", "never") != "always":
            return data
        data = _obj_dict(data)
    if not isinstance(data, dict):
        if _cfg(cls).get("from_attributes", False) and not isinstance(data, (str, bytes, int, float, list, tuple)):
            data = _attrs_of(cls, data)
        else:
            _fail("model_type", data, {"class_name": cls.__name__})
    out, fset, extra = _validate_fields(cls, s, data, st)
    obj = inst if inst is not None else object.__new__(cls)
    for k, v in out.items():
        object.__setattr__(obj, k, v)
    object.__setattr__(obj, "__pydantic_fields_set__", fset)
    object.__setattr__(obj, "__pydantic_extra__", extra)
    _init_private(obj)
    if _I(cls).post_init:
        try:
            obj.model_post_init(st.context)
        except PydanticCustomError as e:
            raise _Err([_Line(e.type, data, e.context, e.message(), True)])
        except ValidationError as e:
            raise _Err([_copy_line(ln) for ln in e._lines])
        except ValueError as e:
            raise _Err([_Line("value_error", data, {"error": e})])
        except AssertionError as e:
            raise _Err([_Line("assertion_error", data, {"error": e})])
    for fn, tc, np in s.after:
        try:
            r = fn(obj, info) if np >= 2 else fn(obj)
        except PydanticCustomError as e:
            raise _Err([_Line(e.type, data, e.context, e.message(), True)])
        except ValidationError as e:
            raise _Err([_copy_line(ln) for ln in e._lines])
        except ValueError as e:
            raise _Err([_Line("value_error", data, {"error": e})])
        except AssertionError as e:
            raise _Err([_Line("assertion_error", data, {"error": e})])
        if r is not None:
            obj = r
    return obj


def _title(cls):
    # the title errors print: model_config's title, else the class name
    t = _cfg(cls).get("title")
    return t if t is not None else cls.__name__


def _attrs_of(cls, o):
    d = {}
    for f in _schema(cls).fields:
        k = f.alias if f.alias is not None else f.name
        if hasattr(o, k):
            d[k] = getattr(o, k)
    return d


def _init_private(obj):
    priv = type(obj).__private_attributes__
    if priv:
        p = {}
        for k, a in priv.items():
            dv = a.get_default()
            if dv is not PydanticUndefined:
                p[k] = dv
        object.__setattr__(obj, "__pydantic_private__", p)
    else:
        object.__setattr__(obj, "__pydantic_private__", None)


_JSON_MSG = {
    "model_type": "Input should be an object",
    "dict_type": "Input should be an object",
    "list_type": "Input should be a valid array",
    "tuple_type": "Input should be a valid array",
    "set_type": "Input should be a valid array",
    "frozen_set_type": "Input should be a valid array",
    "none_required": "Input should be null",
}


def _top(cls, f, json_mode=False):
    # run f() and turn its errors into a ValidationError titled with the class name
    try:
        return f()
    except _Err as er:
        if json_mode:
            for ln in er.lines:
                if ln.t in _JSON_MSG and not ln.custom:
                    ln.msg = _JSON_MSG[ln.t]
        raise ValidationError(_title(cls), er.lines, _cfg(cls).get("hide_input_in_errors", False)) from None


# ---------------------------------------------------------------- dumping


class _DumpCtx:
    def __init__(self, mode, by_alias, exclude_unset, exclude_defaults, exclude_none, round_trip=False):
        self.json = mode == "json"
        self.by_alias = by_alias
        self.exclude_unset = exclude_unset
        self.exclude_defaults = exclude_defaults
        self.exclude_none = exclude_none
        self.inc = None
        self.exc = None


def _sub(spec, key):
    # the include / exclude for one field (or item) out of a set or dict spec: None (all), True (whole), dict
    if spec is None:
        return None
    if isinstance(spec, dict):
        v = spec.get(key, spec.get("__all__"))
        return v
    return True if key in spec else None


def _filter_items(seq, ctx):
    return seq


def _dump_model(m, ctx, cls):
    s = _schema(cls)
    out = {}
    inc = ctx.inc
    exc = ctx.exc
    d = m.__dict__
    for f in s.fields:
        name = f.name
        if inc is not None:
            si = _sub(inc, name)
            if si is None:
                continue
        else:
            si = None
        se = _sub(exc, name) if exc is not None else None
        if se is True:
            continue
        if f.info.exclude:
            continue
        if name not in d:
            continue
        v = d[name]
        if ctx.exclude_unset and name not in m.__pydantic_fields_set__:
            continue
        if ctx.exclude_none and v is None:
            continue
        if ctx.exclude_defaults:
            info = f.info
            if info.default_factory is not None:
                if v == info.default_factory():
                    continue
            elif info.default is not PydanticUndefined and v == info.default:
                continue
        key = f.ser_alias if (ctx.by_alias and f.ser_alias is not None) else name
        out[key] = _nested(f.val, v, ctx, si if si is not True else None, se)
    extra = m.__pydantic_extra__
    if extra:
        for k, v in extra.items():
            if inc is not None and _sub(inc, k) is None:
                continue
            if exc is not None and _sub(exc, k) is True:
                continue
            if ctx.exclude_none and v is None:
                continue
            out[k] = _nested(None, v, ctx, None, None)
    for k, dv in s.computed:
        if inc is not None and _sub(inc, k) is None:
            continue
        if exc is not None and _sub(exc, k) is True:
            continue
        v = getattr(m, k)
        if ctx.exclude_none and v is None:
            continue
        key = dv.extra.get("alias") if (ctx.by_alias and dv.extra.get("alias")) else k
        out[key] = _nested(None, v, ctx, None, None)
    return out


def _nested(val, v, ctx, inc, exc):
    oi, oe = ctx.inc, ctx.exc
    ctx.inc, ctx.exc = inc, exc
    try:
        r = _ser(val, v, ctx)
        if (inc is not None or exc is not None) and isinstance(r, (list, tuple)) and not isinstance(v, BaseModel):
            r = _filter_seq(r, inc, exc)
        elif (inc is not None or exc is not None) and isinstance(r, dict) and not isinstance(v, BaseModel):
            r = {k: e for k, e in r.items() if (inc is None or _sub(inc, k) is not None) and (exc is None or _sub(exc, k) is not True)}
    finally:
        ctx.inc, ctx.exc = oi, oe
    return r


def _ser(val, v, ctx):
    # a value dumped by its declared type, or by what it is when it is not of that type
    if val is None or not val.is_(v):
        return _infer(v, ctx)
    return val.ser(v, ctx)


def _filter_seq(r, inc, exc):
    out = []
    for i in range(len(r)):
        if inc is not None and _sub(inc, i) is None:
            continue
        if exc is not None and _sub(exc, i) is True:
            continue
        out.append(r[i])
    return out if isinstance(r, list) else tuple(out)


def _infer(v, ctx):
    # dumping a value of no declared type: by what it is
    if v is None or v is True or v is False:
        return v
    t = type(v)
    if t is int or t is str:
        return v
    if t is float:
        if ctx.json and (v != v or v in (float("inf"), float("-inf"))):
            return None
        return v
    if isinstance(v, BaseModel):
        return _dump_model(v, ctx, type(v))
    if t is list:
        return [_infer(e, ctx) for e in v]
    if t is tuple:
        out = [_infer(e, ctx) for e in v]
        return out if ctx.json else tuple(out)
    if t is dict:
        out = {}
        for k, e in v.items():
            k2 = _infer(k, ctx)
            if ctx.json:
                k2 = _json_key(k2)
            out[k2] = _infer(e, ctx)
        return out
    if t is set or t is frozenset:
        out = [_infer(e, ctx) for e in v]
        if ctx.json:
            return out
        return set(out) if t is set else frozenset(out)
    if t is bytes:
        return v.decode("utf-8") if ctx.json else v
    if _is_enum(t):
        return _infer(v.value, ctx) if ctx.json else v
    if isinstance(v, (int, float, str)):
        return v
    if ctx.json:
        raise PydanticSerializationError("Unable to serialize unknown type: %r" % (t,))
    return v


class PydanticSerializationError(ValueError):
    def __init__(self, message):
        self.message = message

    def __str__(self):
        return self.message


PydanticSerializationError.__module__ = "pydantic_core._pydantic_core"


def _jfloat(f):
    # a float as pydantic-core's JSON writes it
    if f != f or f in (float("inf"), float("-inf")):
        return "null"
    r = repr(f)
    if "e" not in r and "E" not in r:
        if r.startswith("-"):
            neg, body = "-", r[1:]
        else:
            neg, body = "", r
        # 0.0000ddd: Python writes 1e-05 for 0.00001; repr has no exponent here
        return r
    m, e = r.split("e")
    ex = int(e)
    neg = m.startswith("-")
    if neg:
        m = m[1:]
    digits = m.replace(".", "")
    point = len(m.split(".")[0]) + ex
    if -5 < point <= 0:
        s = "0." + "0" * (-point) + digits
        return ("-" if neg else "") + s
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
    t = type(v)
    if t is str or isinstance(v, str):
        return _jstr(str(v))
    if t is float or isinstance(v, float):
        return _jfloat(float(v))
    if t is int or isinstance(v, int):
        return str(int(v))
    if isinstance(v, dict):
        if not v:
            return "{}"
        if indent is None:
            return "{" + ",".join(_jstr(k) + ":" + _jdump(e) for k, e in v.items()) + "}"
        pad = "\n" + " " * (indent * (level + 1))
        return "{" + ",".join(pad + _jstr(k) + ": " + _jdump(e, indent, level + 1) for k, e in v.items()) + "\n" + " " * (indent * level) + "}"
    if isinstance(v, (list, tuple)):
        if not v:
            return "[]"
        if indent is None:
            return "[" + ",".join(_jdump(e) for e in v) + "]"
        pad = "\n" + " " * (indent * (level + 1))
        return "[" + ",".join(pad + _jdump(e, indent, level + 1) for e in v) + "\n" + " " * (indent * level) + "]"
    raise PydanticSerializationError("Unable to serialize unknown type: %r" % (t,))


def _to_jsonable(v):
    ctx = _DumpCtx("json", False, False, False, False)
    return _infer(v, ctx)


def to_json(value, *, indent=None):
    return _jdump(_to_jsonable(value), indent).encode("utf-8")


# ---------------------------------------------------------------- JSON parsing (pydantic-core's jiter messages)


class _JErr(Exception):
    def __init__(self, msg, pos):
        self.msg = msg
        self.pos = pos


_WS = " \t\n\r"


class _JP:
    def __init__(self, s):
        self.s = s
        self.n = len(s)
        self.i = 0
        self.depth = 0

    def ws(self):
        s = self.s
        i = self.i
        n = self.n
        while i < n and s[i] in _WS:
            i += 1
        self.i = i

    def err(self, msg, pos):
        raise _JErr(msg, pos)

    def value(self, ctx):
        self.ws()
        if self.i >= self.n:
            self.err("EOF while parsing " + ctx, self.n)
        c = self.s[self.i]
        if c == "{":
            return self.obj()
        if c == "[":
            return self.arr()
        if c == '"':
            return self.string()
        if c == "t":
            return self.word("true", True)
        if c == "f":
            return self.word("false", False)
        if c == "n":
            return self.word("null", None)
        if c == "N":
            return self.word("NaN", float("nan"), True)
        if c == "I":
            return self.word("Infinity", float("inf"), True)
        if c == "-" or ("0" <= c <= "9"):
            return self.number()
        self.err("expected value", self.i + 1)

    def word(self, w, v, nonfinite=False):
        s = self.s
        i = self.i
        k = 0
        while k < len(w):
            if i + k >= self.n:
                self.err("EOF while parsing a value", self.n)
            if s[i + k] != w[k]:
                self.err("expected value" if nonfinite else "expected ident", i + k + 1)
            k += 1
        self.i = i + len(w)
        if nonfinite:
            self.err("expected value", i + 1)
        return v

    def number(self):
        s = self.s
        n = self.n
        st = self.i
        i = st
        if s[i] == "-":
            i += 1
            if i >= n:
                self.err("EOF while parsing a value", n)
            if s[i] == "I":
                self.err("expected value", i)
        if i < n and s[i] == "0":
            i += 1
            if i < n and "0" <= s[i] <= "9":
                self.err("invalid number", i + 1)
        elif i < n and "1" <= s[i] <= "9":
            while i < n and "0" <= s[i] <= "9":
                i += 1
        else:
            self.err("invalid number", i + 1)
        isf = False
        if i < n and s[i] == ".":
            isf = True
            i += 1
            if i >= n:
                self.err("EOF while parsing a value", n)
            if not ("0" <= s[i] <= "9"):
                self.err("invalid number", i + 1)
            while i < n and "0" <= s[i] <= "9":
                i += 1
        if i < n and s[i] in "eE":
            isf = True
            i += 1
            if i < n and s[i] in "+-":
                i += 1
            if i >= n:
                self.err("EOF while parsing a value", n)
            if not ("0" <= s[i] <= "9"):
                self.err("invalid number", i + 1)
            while i < n and "0" <= s[i] <= "9":
                i += 1
        self.i = i
        t = s[st:i]
        if isf:
            return float(t)
        return int(t)

    def string(self):
        s = self.s
        n = self.n
        i = self.i + 1
        out = []
        start = i
        while True:
            if i >= n:
                self.err("EOF while parsing a string", n)
            c = s[i]
            if c == '"':
                out.append(s[start:i])
                self.i = i + 1
                return "".join(out)
            if c == "\\":
                out.append(s[start:i])
                i += 1
                if i >= n:
                    self.err("EOF while parsing a string", n)
                e = s[i]
                if e == "n":
                    out.append("\n")
                elif e == "t":
                    out.append("\t")
                elif e == "r":
                    out.append("\r")
                elif e == "b":
                    out.append("\b")
                elif e == "f":
                    out.append("\f")
                elif e == "/":
                    out.append("/")
                elif e == "\\":
                    out.append("\\")
                elif e == '"':
                    out.append('"')
                elif e == "u":
                    cp, i = self.hex4(i)
                    if 0xD800 <= cp < 0xDC00:
                        if i + 2 < n and s[i + 1] == "\\" and s[i + 2] == "u":
                            lo, j = self.hex4(i + 2)
                            if 0xDC00 <= lo < 0xE000:
                                cp = 0x10000 + ((cp - 0xD800) << 10) + (lo - 0xDC00)
                                i = j
                            else:
                                self.err("lone leading surrogate in hex escape", j + 1)
                        else:
                            self.err("unexpected end of hex escape", i + 2)
                    elif 0xDC00 <= cp < 0xE000:
                        self.err("invalid unicode code point", i + 1)
                    out.append(chr(cp))
                else:
                    self.err("invalid escape", i + 1)
                i += 1
                start = i
                continue
            if c < " ":
                self.err("control character (\\u0000-\\u001F) found while parsing a string", i + 1)
            i += 1

    def hex4(self, i):
        s = self.s
        v = 0
        for k in range(1, 5):
            if i + k >= self.n:
                self.err("EOF while parsing a string", self.n)
            c = s[i + k]
            d = "0123456789abcdef".find(c.lower())
            if d < 0 or len(c) != 1:
                self.err("invalid escape", i + k + 1)
            v = v * 16 + d
        return v, i + 4

    def arr(self):
        self.i += 1
        self.depth += 1
        if self.depth > 200:
            self.err("recursion limit exceeded", self.i)
        out = []
        self.ws()
        if self.i >= self.n:
            self.err("EOF while parsing a list", self.n)
        if self.s[self.i] == "]":
            self.i += 1
            self.depth -= 1
            return out
        while True:
            out.append(self.value("a value"))
            self.ws()
            if self.i >= self.n:
                self.err("EOF while parsing a list", self.n)
            c = self.s[self.i]
            if c == ",":
                self.i += 1
                self.ws()
                if self.i < self.n and self.s[self.i] == "]":
                    self.err("trailing comma", self.i + 1)
                continue
            if c == "]":
                self.i += 1
                self.depth -= 1
                return out
            self.err("expected `,` or `]`", self.i + 1)

    def obj(self):
        self.i += 1
        self.depth += 1
        if self.depth > 200:
            self.err("recursion limit exceeded", self.i)
        out = {}
        self.ws()
        if self.i >= self.n:
            self.err("EOF while parsing an object", self.n)
        if self.s[self.i] == "}":
            self.i += 1
            self.depth -= 1
            return out
        while True:
            self.ws()
            if self.i >= self.n:
                self.err("EOF while parsing an object", self.n)
            if self.s[self.i] != '"':
                self.err("key must be a string", self.i + 1)
            k = self.string()
            self.ws()
            if self.i >= self.n:
                self.err("EOF while parsing an object", self.n)
            if self.s[self.i] != ":":
                self.err("expected `:`", self.i + 1)
            self.i += 1
            out[k] = self.value("a value")
            self.ws()
            if self.i >= self.n:
                self.err("EOF while parsing an object", self.n)
            c = self.s[self.i]
            if c == ",":
                self.i += 1
                self.ws()
                if self.i < self.n and self.s[self.i] == "}":
                    self.err("trailing comma", self.i + 1)
                continue
            if c == "}":
                self.i += 1
                self.depth -= 1
                return out
            self.err("expected `,` or `}`", self.i + 1)


def _jparse(s):
    p = _JP(s)
    v = p.value("a value")
    p.ws()
    if p.i < p.n:
        p.err("trailing characters", p.i + 1)
    return v


def _jpos(s, pos):
    # line and column of a position, as jiter reports them
    head = s[:pos]
    line = head.count("\n") + 1
    last = head.rfind("\n")
    return line, len(head[last + 1:].encode("utf-8"))


def _from_json(data):
    if isinstance(data, bytes):
        try:
            data = data.decode("utf-8")
        except UnicodeDecodeError:
            _fail("json_invalid", data, {"error": "invalid unicode code point"})
    elif not isinstance(data, str):
        _fail("json_type", data)
    try:
        return _jparse(data)
    except _JErr as e:
        line, col = _jpos(data, e.pos)
        _fail("json_invalid", data, {"error": "%s at line %d column %d" % (e.msg, line, col)})


# ---------------------------------------------------------------- BaseModel


class BaseModel:
    __slots__ = ("__dict__", "__pydantic_fields_set__", "__pydantic_extra__", "__pydantic_private__")
    model_config = {}
    model_fields = {}
    model_computed_fields = {}
    __class_vars__ = set()
    __private_attributes__ = {}

    def __init_subclass__(cls, **kw):
        _setup(cls)

    def __init__(self, /, **data):
        cls = type(self)
        st = _St(strict=_cfg(cls).get("strict", False))
        _top(cls, lambda: _validate_model(cls, data, st, self))

    @classmethod
    def model_rebuild(cls, *, force=False, raise_errors=True, _parent_namespace_depth=2, _types_namespace=None):
        # Brevi resolves a forward reference when it is first used, so there is nothing to rebuild. pydantic
        # answers True for a model it had to leave incomplete (one naming a class defined later), else None.
        _I(cls)
        if force or any(_has_lazy(d) for d in _I(cls).descs.values()):
            return True
        return None

    @classmethod
    def model_validate(cls, obj, *, strict=None, from_attributes=None, context=None, by_alias=None, by_name=None):
        st = _St(strict=_cfg(cls).get("strict", False) if strict is None else strict, context=context)
        if from_attributes and not isinstance(obj, (dict, BaseModel)):
            return _top(cls, lambda: _validate_model(cls, _attrs_of(cls, obj), st, None))
        return _top(cls, lambda: _validate_model(cls, obj, st, None))

    @classmethod
    def model_validate_json(cls, json_data, *, strict=None, context=None, by_alias=None, by_name=None):
        st = _St(strict=_cfg(cls).get("strict", False) if strict is None else strict, mode="json", context=context)

        def run():
            data = _from_json(json_data)
            return _validate_model(cls, data, st, None)
        return _top(cls, run, True)

    @classmethod
    def model_json_schema(cls, by_alias=True, ref_template="#/$defs/{model}", mode="validation", *, union_format="any_of"):
        return _js_top(("cls", cls), _JsDefs(ref_template, by_alias))

    @classmethod
    def model_validate_strings(cls, obj, *, strict=None, context=None):
        return cls.model_validate(obj, strict=strict, context=context)

    @classmethod
    def model_construct(cls, _fields_set=None, **values):
        obj = object.__new__(cls)
        s = _schema(cls)
        fset = set()
        for f in s.fields:
            k = f.alias if (f.alias is not None and f.alias in values) else f.name
            if k in values:
                object.__setattr__(obj, f.name, values[k])
                fset.add(f.name)
            elif f.info.default_factory is not None:
                object.__setattr__(obj, f.name, f.info.default_factory())
            elif f.info.default is not PydanticUndefined:
                object.__setattr__(obj, f.name, _copy_default(f.info.default))
        extra = None
        if _cfg(cls).get("extra") == "allow":
            extra = {}
            for k, v in values.items():
                if k not in s.by_name:
                    extra[k] = v
                    fset.add(k)
        object.__setattr__(obj, "__pydantic_fields_set__", set(_fields_set) if _fields_set is not None else fset)
        object.__setattr__(obj, "__pydantic_extra__", extra)
        _init_private(obj)
        return obj

    def model_dump(self, *, mode="python", include=None, exclude=None, context=None, by_alias=None, exclude_unset=False,
                   exclude_defaults=False, exclude_none=False, exclude_computed_fields=False, round_trip=False, warnings=True,
                   fallback=None, serialize_as_any=False):
        ctx = _DumpCtx(mode, bool(by_alias), exclude_unset, exclude_defaults, exclude_none, round_trip)
        ctx.inc = include
        ctx.exc = exclude
        return _dump_model(self, ctx, type(self))

    def model_dump_json(self, *, indent=None, ensure_ascii=False, include=None, exclude=None, context=None, by_alias=None,
                        exclude_unset=False, exclude_defaults=False, exclude_none=False, exclude_computed_fields=False,
                        round_trip=False, warnings=True, fallback=None, serialize_as_any=False):
        ctx = _DumpCtx("json", bool(by_alias), exclude_unset, exclude_defaults, exclude_none, round_trip)
        ctx.inc = include
        ctx.exc = exclude
        return _jdump(_dump_model(self, ctx, type(self)), indent)

    def model_copy(self, *, update=None, deep=False):
        cls = type(self)
        obj = object.__new__(cls)
        d = dict(self.__dict__)
        if deep:
            d = {k: _deep(v) for k, v in d.items()}
        extra = self.__pydantic_extra__
        extra = dict(extra) if extra is not None else None
        fset = set(self.__pydantic_fields_set__)
        if update:
            for k, v in update.items():
                if k in _I(cls).fields:
                    d[k] = v
                elif extra is not None:
                    extra[k] = v
                else:
                    d[k] = v
                fset.add(k)
        for k, v in d.items():
            object.__setattr__(obj, k, v)
        object.__setattr__(obj, "__pydantic_fields_set__", fset)
        object.__setattr__(obj, "__pydantic_extra__", extra)
        p = self.__pydantic_private__
        object.__setattr__(obj, "__pydantic_private__", (_deep(p) if deep else dict(p)) if p is not None else None)
        return obj

    @property
    def model_fields_set(self):
        return self.__pydantic_fields_set__

    @property
    def model_extra(self):
        return self.__pydantic_extra__

    def model_post_init(self, context):
        pass

    def __repr_args__(self):
        cls = type(self)
        fields = _I(cls).fields
        out = []
        for k, v in self.__dict__.items():
            f = fields.get(k)
            if f is not None and f.repr:
                out.append((k, v))
        extra = self.__pydantic_extra__
        if extra:
            for k, v in extra.items():
                out.append((k, v))
        for k, dv in _schema(cls).computed:
            if dv.extra.get("repr", True):
                out.append((k, getattr(self, k)))
        return out

    def __repr__(self):
        return "%s(%s)" % (type(self).__name__, ", ".join("%s=%s" % (k, _r(v, self)) for k, v in self.__repr_args__()))

    def __str__(self):
        return " ".join("%s=%s" % (k, _r(v, self)) for k, v in self.__repr_args__())

    def __iter__(self):
        for k, v in self.__dict__.items():
            yield k, v
        extra = self.__pydantic_extra__
        if extra:
            for k, v in extra.items():
                yield k, v

    def __eq__(self, other):
        if isinstance(other, BaseModel):
            return type(self) is type(other) and self.__dict__ == other.__dict__ and self.__pydantic_private__ == other.__pydantic_private__ and self.__pydantic_extra__ == other.__pydantic_extra__
        return NotImplemented

    def __getattr__(self, name):
        priv = type(self).__private_attributes__
        if name in priv:
            p = object.__getattribute__(self, "__pydantic_private__")
            if p is not None and name in p:
                return p[name]
        extra = None
        try:
            extra = object.__getattribute__(self, "__pydantic_extra__")
        except AttributeError:
            pass
        if extra is not None and name in extra:
            return extra[name]
        raise AttributeError("%r object has no attribute %r" % (type(self).__name__, name))

    def __setattr__(self, name, value):
        cls = type(self)
        if name in cls.__class_vars__:
            raise AttributeError("%r is a ClassVar of `%s` and cannot be set on an instance. If you want to set a value on the class, use `%s.%s = value`." % (name, cls.__name__, cls.__name__, name))
        if name.startswith("_"):
            priv = cls.__private_attributes__
            if name in priv:
                p = self.__pydantic_private__
                if p is None:
                    p = {}
                    object.__setattr__(self, "__pydantic_private__", p)
                p[name] = value
                return
            object.__setattr__(self, name, value)
            return
        cfg = _cfg(cls)
        fields = _I(cls).fields
        if cfg.get("frozen", False):
            raise ValidationError(cls.__name__, _at([_Line("frozen_instance", value)], name))
        f = fields.get(name)
        if f is not None and f.frozen:
            raise ValidationError(cls.__name__, _at([_Line("frozen_field", value)], name))
        attr = getattr(cls, name, None)
        if attr is not None and _is_prop(attr):
            if attr.fset is None:
                raise AttributeError("property %r of %r object has no setter" % (name, cls.__name__))
            attr.fset(self, value)
            return
        if f is None:
            if cfg.get("extra") == "allow":
                self.__pydantic_extra__[name] = value
                self.__pydantic_fields_set__.add(name)
                return
            if cfg.get("validate_assignment", False):
                raise ValidationError(_title(cls), _at([_Line("no_such_attribute", value, {"attribute": name})], name))
            raise ValueError('"%s" object has no field "%s"' % (cls.__name__, name))
        if cfg.get("validate_assignment", False):
            s = _schema(cls)
            st = _St(strict=cfg.get("strict", False))
            data = {k: v for k, v in self.__dict__.items() if k != name}

            def run():
                try:
                    return _run_field(cls, s.by_name[name], value, st, data)
                except _Err as er:
                    raise _Err(_at(er.lines, name))
            value = _top(cls, run)
            object.__setattr__(self, name, value)
            self.__pydantic_fields_set__.add(name)
            for fn, tc, np in s.after:
                r = fn(self, ValidationInfo({}, None, "python", None, cls.model_config)) if np >= 2 else fn(self)
            return
        object.__setattr__(self, name, value)
        self.__pydantic_fields_set__.add(name)

    def __hash__(self):
        cls = type(self)
        if not _cfg(cls).get("frozen", False):
            raise TypeError("unhashable type: %r" % cls.__name__)
        vals = [self.__dict__[k] for k in _I(cls).fields if k in self.__dict__]
        return hash(vals[0] if len(vals) == 1 else tuple(vals))

    def __copy__(self):
        return self.model_copy()

    def __deepcopy__(self, memo=None):
        return self.model_copy(deep=True)

    def dict(self, **kw):
        return self.model_dump(**kw)

    def json(self, **kw):
        return self.model_dump_json(**kw)


def _is_prop(v):
    return hasattr(v, "fget") and hasattr(v, "fset")


def _r(v, me):
    if v is me:
        return "Self"
    return repr(v)


def _deep(v):
    t = type(v)
    if t is list:
        return [_deep(e) for e in v]
    if t is dict:
        return {k: _deep(e) for k, e in v.items()}
    if t is set:
        return {_deep(e) for e in v}
    if t is tuple:
        return tuple(_deep(e) for e in v)
    if isinstance(v, BaseModel):
        return v.model_copy(deep=True)
    return v



# ---------------------------------------------------------------- JSON schema (model_json_schema, TypeAdapter.json_schema)
# pydantic's GenerateJsonSchema for the descriptors this copy has: models and enums go to $defs (referenced by
# ref_template), keys sorted as pydantic sorts them (not inside properties or a default), field titles from the
# names, defaults in their JSON form. Brevi's mcp builds its tool schemas with these too.


class _NoJsonSchema(PydanticSchemaGenerationError):
    pass


def _js_title(name):
    # pydantic's default field title
    return name.title().replace("_", " ")


def _js_sort(v, parent=None):
    if isinstance(v, dict):
        keys = list(v.keys())
        if parent not in ("properties", "default"):
            keys = sorted(keys)
        out = {}
        for k in keys:
            out[k] = _js_sort(v[k], k)
        return out
    if isinstance(v, list):
        return [_js_sort(x, parent) for x in v]
    return v


class _JsDefs:
    def __init__(self, ref_template="#/$defs/{model}", by_alias=True):
        self.defs = {}
        self.building = set()
        self.ref_template = ref_template
        self.by_alias = by_alias
        # the names a $ref was made to (a model that refers to itself goes to $defs too)
        self.refd = set()

    def ref(self, name):
        self.refd.add(name)
        return {"$ref": self.ref_template.replace("{model}", name)}


def _js_jsonable(v):
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
    if isinstance(v, (list, tuple, set, frozenset)):
        return [_js_jsonable(e) for e in v]
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
                k2 = _jdump(k)
            else:
                k2 = str(k)
            out[k2] = _js_jsonable(e)
        return out
    if isinstance(v, bytes):
        return v.decode("utf-8")
    if isinstance(v, int):
        return int(v)
    if isinstance(v, float):
        return float(v)
    if isinstance(v, str):
        return str(v)
    if _js_is_enum_value(v):
        return _js_jsonable(v.value)
    if hasattr(v, "__dataclass_fields__"):
        out = {}
        for k in v.__dataclass_fields__:
            out[k] = _js_jsonable(getattr(v, k))
        return out
    return str(v)


def _js_is_enum_value(v):
    for b in type(v).__mro__:
        if b.__name__ == "Enum" and b.__module__ == "enum":
            return True
    return False


def _js_cons(metas):
    # the constraints and field info in Annotated metadata / Field(...)
    cons = {}
    info = {}
    for m in metas:
        if isinstance(m, FieldInfo):
            for k in ("title", "description", "examples", "default", "default_factory", "alias", "deprecated", "json_schema_extra"):
                v = getattr(m, k, None)
                if k == "default":
                    if v is not PydanticUndefined:
                        info.setdefault("default", v)
                    continue
                if v is not None and k not in info:
                    info[k] = v
            for mm in m.metadata:
                _js_meta(mm, cons)
        else:
            _js_meta(m, cons)
    return cons, info


def _js_meta(m, cons):
    if isinstance(m, _Meta):
        cons[m.key] = m.value
    elif isinstance(m, _General):
        for k, v in m.items:
            cons[k] = v


_JS_NUM = {"gt": "exclusiveMinimum", "ge": "minimum", "lt": "exclusiveMaximum", "le": "maximum", "multiple_of": "multipleOf"}


def _js_apply(s, cons, kind):
    for k, v in cons.items():
        if k in _JS_NUM and kind in ("int", "float"):
            s[_JS_NUM[k]] = v
        elif k == "min_length":
            s["minLength" if kind in ("str", "bytes") else ("minProperties" if kind == "dict" else "minItems")] = v
        elif k == "max_length":
            s["maxLength" if kind in ("str", "bytes") else ("maxProperties" if kind == "dict" else "maxItems")] = v
        elif k == "pattern" and kind == "str":
            s["pattern"] = v
    return s


def _js_types(vals):
    ts = set()
    for v in vals:
        if v is None:
            ts.add("null")
        elif isinstance(v, bool):
            ts.add("boolean")
        elif isinstance(v, int):
            ts.add("integer")
        elif isinstance(v, float):
            ts.add("number")
        elif isinstance(v, str):
            ts.add("string")
        else:
            ts.add("?")
    return ts


def _js_schema(d, cx, metas=()):
    t = d[0]
    if t == "ann":
        return _js_schema(d[1], cx, tuple(d[2]) + tuple(metas))
    cons, info = _js_cons(metas)
    if t == "cls":
        v = d[1]
        if isinstance(v, tuple):
            return _js_schema(v, cx, metas)
        return _js_cls(v, cx, cons)
    if t == "lazy":
        return _js_cls(d[1](), cx, cons)
    if t in ("int", "float", "str", "bool", "bytes", "none"):
        s = {"int": {"type": "integer"}, "float": {"type": "number"}, "str": {"type": "string"}, "bool": {"type": "boolean"},
             "bytes": {"format": "binary", "type": "string"}, "none": {"type": "null"}}[t]
        return _js_apply(dict(s), cons, t)
    if t == "any":
        return {}
    if t in ("list", "set", "frozenset", "tuplevar"):
        s = {"items": _js_schema(d[1], cx), "type": "array"}
        if t in ("set", "frozenset"):
            s["uniqueItems"] = True
        return _js_apply(s, cons, "list")
    if t == "tuple":
        n = len(d[1])
        s = {"maxItems": n, "minItems": n, "type": "array"}
        if n:
            s["prefixItems"] = [_js_schema(x, cx) for x in d[1]]
        return s
    if t == "dict":
        if d[4]:
            s = {"additionalProperties": True, "type": "object"}
        else:
            s = {"additionalProperties": _js_schema(d[2], cx), "type": "object"}
        return _js_apply(s, cons, "dict")
    if t == "union":
        arms = []
        for x in d[1]:
            sx = _js_schema(x, cx)
            if sx not in arms:
                arms.append(sx)
        if len(arms) == 1:
            return arms[0]
        return {"anyOf": arms}
    if t == "literal":
        vals = [(v.value if _js_is_enum_value(v) else v) for v in d[1]]
        ts = _js_types(vals)
        s = {}
        if len(vals) == 1:
            s["const"] = vals[0]
        else:
            s["enum"] = vals
        if len(ts) == 1 and "?" not in ts:
            s["type"] = list(ts)[0]
        return s
    raise _NoJsonSchema("no JSON schema for %r" % (d,))


def _js_cls(c, cx, cons):
    if c is int:
        return _js_apply({"type": "integer"}, cons, "int")
    if c is float:
        return _js_apply({"type": "number"}, cons, "float")
    if c is str:
        return _js_apply({"type": "string"}, cons, "str")
    if c is bool:
        return {"type": "boolean"}
    if c is bytes:
        return _js_apply({"format": "binary", "type": "string"}, cons, "bytes")
    if c is type(None):
        return {"type": "null"}
    if c is object:
        return {}
    if isinstance(c, type) and issubclass(c, BaseModel):
        name = c.__name__
        if name not in cx.defs and name not in cx.building:
            cx.building.add(name)
            cx.defs[name] = _js_model(c, cx)
            cx.building.discard(name)
        return cx.ref(name)
    if isinstance(c, type) and _is_enum(c):
        name = c.__name__
        if name not in cx.defs:
            vals = [m.value for m in c]
            s = {"enum": vals, "title": name}
            ts = _js_types(vals)
            if len(ts) == 1 and "?" not in ts:
                s["type"] = list(ts)[0]
            doc = c.__dict__.get("__doc__")
            if doc and doc != "An enumeration.":
                s["description"] = _js_doc(doc)
            cx.defs[name] = s
        return cx.ref(name)
    raise _NoJsonSchema("no JSON schema for %r" % (c,))


def _js_doc(doc):
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


def _js_model(cls, cx):
    _schema(cls)
    info = _INFO[cls]
    props = {}
    req = []
    for name, f in info.fields.items():
        key = f.alias if (f.alias is not None and cx.by_alias) else name
        props[key] = _js_prop(name, f._desc, f, cx)
        if f.default is PydanticUndefined and f.default_factory is None:
            req.append(key)
    s = {"properties": props}
    if req:
        s["required"] = req
    s["title"] = info.cfg.get("title") or cls.__name__
    s["type"] = "object"
    doc = cls.__dict__.get("__doc__")
    if doc:
        s["description"] = _js_doc(doc)
    if info.cfg.get("extra") == "forbid":
        s["additionalProperties"] = False
    return s


def _js_refish(d):
    # whether a field of descriptor d gets no title: pydantic sets none on a model or enum (they have a ref),
    # also when Optional
    while d[0] == "ann" or (d[0] == "cls" and isinstance(d[1], tuple)):
        d = d[1]
    if d[0] == "union":
        arms = [x for x in d[1] if x[0] != "none"]
        if len(arms) == 1 and len(arms) < len(d[1]):
            return _js_refish(arms[0])
        return False
    if d[0] == "lazy":
        return True
    if d[0] == "cls":
        c = d[1]
        return isinstance(c, type) and (issubclass(c, BaseModel) or _is_enum(c))
    return False


def _js_prop(name, d, f, cx):
    # one field: its type's schema with the field's title, default, description, ...
    metas = list(f.metadata) if f is not None else []
    s = dict(_js_schema(d, cx, metas))
    title = getattr(f, "title", None) if f is not None else None
    if title is not None:
        s["title"] = title
    elif "$ref" not in s and not _js_refish(d):
        s["title"] = _js_title(name)
    if f is not None:
        if f.description is not None:
            s["description"] = f.description
        if f.default is not PydanticUndefined:
            s["default"] = _js_jsonable(f.default)
        if getattr(f, "examples", None) is not None:
            s["examples"] = _js_jsonable(f.examples)
        if getattr(f, "deprecated", None):
            s["deprecated"] = True
        extra = getattr(f, "json_schema_extra", None)
        if isinstance(extra, dict):
            s.update(extra)
    return s


def _js_top(d, cx):
    # the schema of a type at the top: a model's own schema (in $defs with a $ref when it refers to itself)
    c = d[1] if d[0] == "cls" and not isinstance(d[1], tuple) else None
    if isinstance(c, type) and issubclass(c, BaseModel):
        name = c.__name__
        cx.building.add(name)
        s = _js_model(c, cx)
        cx.building.discard(name)
        if name in cx.refd:
            cx.defs[name] = s
            s = cx.ref(name)
    else:
        s = dict(_js_schema(d, cx))
    if cx.defs:
        s["$defs"] = cx.defs
    return _js_sort(s)


# ---------------------------------------------------------------- TypeAdapter


class TypeAdapter:
    # validation, dumping and the JSON schema of a type that is not a model. The compiler hands the type over as
    # a descriptor (src/dynpy/pyd.rs), as it does for model fields; a class (a model, int) comes as itself.
    def __init__(self, type, *, config=None, _parent_depth=2, module=None):
        d = type
        if not isinstance(d, tuple):
            d = ("cls", d)
        self._desc = d
        self._cfg = dict(config) if config else {}
        self._val = None

    def _v(self):
        if self._val is None:
            self._val = _build(self._desc, _Ctx(self._cfg))
        return self._val

    def _run(self, f, json_mode=False):
        v = self._v()
        try:
            return f(v)
        except _Err as er:
            if json_mode:
                for ln in er.lines:
                    if ln.t in _JSON_MSG and not ln.custom:
                        ln.msg = _JSON_MSG[ln.t]
            raise ValidationError(_ta_title(v), er.lines, self._cfg.get("hide_input_in_errors", False)) from None

    def _st(self, strict, context, mode="python"):
        return _St(strict=self._cfg.get("strict", False) if strict is None else strict, mode=mode, context=context)

    def validate_python(self, object, /, *, strict=None, extra=None, from_attributes=None, context=None, experimental_allow_partial=False,
                        by_alias=None, by_name=None):
        st = self._st(strict, context)
        return self._run(lambda v: v.v(object, st))

    def validate_json(self, data, /, *, strict=None, extra=None, context=None, experimental_allow_partial=False, by_alias=None, by_name=None):
        st = self._st(strict, context, "json")
        return self._run(lambda v: v.v(_from_json(data), st), True)

    def validate_strings(self, obj, /, *, strict=None, extra=None, context=None, experimental_allow_partial=False, by_alias=None, by_name=None):
        st = self._st(strict, context, "json")
        return self._run(lambda v: v.v(obj, st), True)

    def dump_python(self, instance, /, *, mode="python", include=None, exclude=None, by_alias=None, exclude_unset=False,
                    exclude_defaults=False, exclude_none=False, exclude_computed_fields=False, round_trip=False, warnings=True,
                    fallback=None, serialize_as_any=False, context=None):
        ctx = _DumpCtx(mode, bool(by_alias), exclude_unset, exclude_defaults, exclude_none, round_trip)
        return _nested(self._v(), instance, ctx, include, exclude)

    def dump_json(self, instance, /, *, indent=None, ensure_ascii=False, include=None, exclude=None, by_alias=None, exclude_unset=False,
                  exclude_defaults=False, exclude_none=False, exclude_computed_fields=False, round_trip=False, warnings=True,
                  fallback=None, serialize_as_any=False, context=None):
        ctx = _DumpCtx("json", bool(by_alias), exclude_unset, exclude_defaults, exclude_none, round_trip)
        return _jdump(_nested(self._v(), instance, ctx, include, exclude), indent).encode()

    def json_schema(self, *, by_alias=True, ref_template="#/$defs/{model}", union_format="any_of", mode="validation"):
        return _js_top(self._desc, _JsDefs(ref_template, by_alias))


def _ta_title(v):
    # pydantic-core titles a TypeAdapter's errors with its validator's name; a model's are the model's title
    if isinstance(v, _Model):
        return _title(v.cls)
    return v.name()

# the constrained types of pydantic.types, as annotation descriptors
def _c(t, name, key, v):
    return ("ann", (t,), (_Meta(name, key, v),))


PositiveInt = _c("int", "Gt", "gt", 0)
NegativeInt = _c("int", "Lt", "lt", 0)
NonNegativeInt = _c("int", "Ge", "ge", 0)
NonPositiveInt = _c("int", "Le", "le", 0)
PositiveFloat = _c("float", "Gt", "gt", 0)
NegativeFloat = _c("float", "Lt", "lt", 0)
NonNegativeFloat = _c("float", "Ge", "ge", 0)
NonPositiveFloat = _c("float", "Le", "le", 0)
FiniteFloat = ("ann", ("float",), (_General([("allow_inf_nan", False)]),))
StrictInt = ("ann", ("int",), (_StrictMark(),))
StrictFloat = ("ann", ("float",), (_StrictMark(),))
StrictStr = ("ann", ("str",), (_StrictMark(),))
StrictBool = ("ann", ("bool",), (_StrictMark(),))
StrictBytes = ("ann", ("bytes",), (_StrictMark(),))
