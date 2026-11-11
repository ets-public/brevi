# Writes the response types of Brevi's anthropic and openai copies (src/dynpy/py/anthropic_types.py and
# openai_types.py) by reading the SDKs' own pydantic models: each class with its fields in the SDK's order, the
# API name of each (alias), its type as a small descriptor and its default, so that what a program sees (repr,
# model_dump, to_json, isinstance, attribute access) matches the SDK's.
#
#   python3 tools/gen_llm_types.py      (needs anthropic 2.6.0 and openai 3.49.0 installed)
#
# Descriptors: None any value; "str" "int" "float" "bool" "none" "dt" (datetime) "date"; ("L", d) list;
# ("D", d) dict values; ("Lit", values); ("M", "Class"); ("U", (d, ...), disc) a union, disc = (field, {value:
# "Class"}) when the SDK names the field that tells its variants apart.
import datetime
import json
import importlib
import inspect
import sys
import types as pytypes
import typing
from typing import get_args, get_origin

import pydantic
from pydantic_core import PydanticUndefined

ROOTS = {
    "anthropic": [
        "anthropic.types.Message",
        "anthropic.types.MessageTokensCount",
        "anthropic.types.RawMessageStreamEvent",
        "anthropic.types.ModelInfo",
        "anthropic.types.ParsedMessage",
        "anthropic.types.shared.ErrorDetails",
        "anthropic.types.MessageDeltaUsage",
        "anthropic.lib.streaming._types.TextEvent",
        "anthropic.lib.streaming._types.CitationEvent",
        "anthropic.lib.streaming._types.ThinkingEvent",
        "anthropic.lib.streaming._types.SignatureEvent",
        "anthropic.lib.streaming._types.InputJsonEvent",
        "anthropic.lib.streaming._types.MessageStopEvent",
        "anthropic.lib.streaming._types.ContentBlockStopEvent",
        "anthropic.lib.streaming._types.ParsedMessageStopEvent",
        "anthropic.lib.streaming._types.ParsedContentBlockStopEvent",
    ],
    "openai": [
        "openai.types.chat.ChatCompletion",
        "openai.types.chat.ChatCompletionChunk",
        "openai.types.CreateEmbeddingResponse",
        "openai.types.Model",
        "openai.types.responses.Response",
        "openai.types.responses.ResponseStreamEvent",
        "openai.types.chat.ParsedChatCompletion",
        "openai.types.responses.ParsedResponse",
        "openai.lib.streaming.chat._events.ChunkEvent",
        "openai.lib.streaming.chat._events.ContentDeltaEvent",
        "openai.lib.streaming.chat._events.ContentDoneEvent",
        "openai.lib.streaming.chat._events.RefusalDeltaEvent",
        "openai.lib.streaming.chat._events.RefusalDoneEvent",
        "openai.lib.streaming.chat._events.FunctionToolCallArgumentsDeltaEvent",
        "openai.lib.streaming.chat._events.FunctionToolCallArgumentsDoneEvent",
        "openai.lib.streaming.chat._events.LogprobsContentDeltaEvent",
        "openai.lib.streaming.chat._events.LogprobsContentDoneEvent",
        "openai.lib.streaming.chat._events.LogprobsRefusalDeltaEvent",
        "openai.lib.streaming.chat._events.LogprobsRefusalDoneEvent",
        "openai.lib.streaming.responses._events.ResponseTextDeltaEvent",
        "openai.lib.streaming.responses._events.ResponseTextDoneEvent",
        "openai.lib.streaming.responses._events.ResponseFunctionCallArgumentsDeltaEvent",
        "openai.lib.streaming.responses._events.ResponseCompletedEvent",
    ],
}


def resolve(spec):
    m, n = spec.rsplit(".", 1)
    return getattr(importlib.import_module(m), n)


class Gen:
    def __init__(self, pkg):
        self.pkg = pkg
        self.classes = {}  # class -> name
        self.order = []
        self.names = {}

    def name_of(self, cls):
        n = cls.__name__.split("[")[0]
        if n in self.names and self.names[n] is not cls:
            # two SDK classes of one name (in different modules): the later one is renamed
            k = 2
            while f"{n}_{k}" in self.names and self.names[f"{n}_{k}"] is not cls:
                k += 1
            n = f"{n}_{k}"
        self.names[n] = cls
        return n

    def model(self, cls):
        if cls in self.classes:
            return self.classes[cls]
        if getattr(cls, "__pydantic_generic_metadata__", None) and cls.__pydantic_generic_metadata__.get("origin"):
            cls = cls.__pydantic_generic_metadata__["origin"]
            if cls in self.classes:
                return self.classes[cls]
        n = self.name_of(cls)
        self.classes[cls] = n
        if not cls.__pydantic_complete__:
            cls.model_rebuild(raise_errors=False)
        fields = []
        for fname, f in cls.model_fields.items():
            fields.append((fname, f.alias, self.desc(f.annotation, list(f.metadata)), self.default(f)))
        bases = [b for b in cls.__mro__[1:] if inspect.isclass(b) and issubclass(b, pydantic.BaseModel) and b.__module__.startswith(self.pkg + ".")]
        base = None
        for b in bases:
            if b.__name__ in ("BaseModel", "GenericModel"):
                continue
            base = b
            break
        if base is not None:
            base = self.model(base)
        self.order.append((n, cls, base, fields))
        return n

    def default(self, f):
        if f.default_factory is not None:
            v = f.default_factory()
            return ("f", type(v).__name__)
        if f.default is PydanticUndefined:
            return ("r",)
        return ("v", f.default)

    def desc(self, t, meta=()):
        o = get_origin(t)
        if o is typing.Annotated:
            args = get_args(t)
            return self.desc(args[0], list(args[1:]) + list(meta))
        if type(t).__name__ == "TypeAliasType":
            return self.desc(t.__value__, meta)
        if t is typing.Any or t is object:
            return None
        if isinstance(t, typing.TypeVar):
            return None
        if o in (typing.Union, pytypes.UnionType):
            ds = tuple(self.desc(a) for a in get_args(t))
            disc = None
            for m in meta:
                fn = getattr(m, "field_name", None) or getattr(m, "discriminator", None)
                if isinstance(fn, str):
                    disc = fn
            if disc is not None:
                mapping = {}
                for a, d in zip(get_args(t), ds):
                    a2 = a
                    while get_origin(a2) is typing.Annotated:
                        a2 = get_args(a2)[0]
                    if inspect.isclass(a2) and issubclass(a2, pydantic.BaseModel):
                        a2.model_rebuild(raise_errors=False) if not a2.__pydantic_complete__ else None
                        ft = a2.model_fields.get(disc)
                        if ft is not None and get_origin(ft.annotation) is typing.Literal:
                            for v in get_args(ft.annotation):
                                mapping.setdefault(v, d[1])
                disc = (disc, mapping)
            return ("U", ds, disc)
        if o is typing.Literal:
            return ("Lit", get_args(t))
        if o in (list, typing.List, typing.Sequence) or t is list:
            a = get_args(t)
            return ("L", self.desc(a[0]) if a else None)
        if o in (dict, typing.Dict) or t is dict:
            a = get_args(t)
            return ("D", self.desc(a[1]) if a else None)
        if t is type(None):
            return "none"
        if t is str:
            return "str"
        if t is bool:
            return "bool"
        if t is int:
            return "int"
        if t is float:
            return "float"
        if t is datetime.datetime:
            return "dt"
        if t is datetime.date:
            return "date"
        if inspect.isclass(t) and issubclass(t, pydantic.BaseModel):
            return ("M", self.model(t))
        if o is not None and inspect.isclass(o) and issubclass(o, pydantic.BaseModel):
            return ("M", self.model(o))
        raise SystemExit(f"type {t!r} has no descriptor")


def walk_roots(g, t):
    o = get_origin(t)
    if o is typing.Annotated:
        return walk_roots(g, get_args(t)[0])
    if type(t).__name__ == "TypeAliasType":
        return walk_roots(g, t.__value__)
    if o in (typing.Union, pytypes.UnionType):
        for a in get_args(t):
            walk_roots(g, a)
        return
    if inspect.isclass(t) and issubclass(t, pydantic.BaseModel):
        g.model(t)


def lit(v):
    if isinstance(v, float) and v != v:
        return 'float("nan")'
    return repr(v)


def dlit(d):
    if d is None or isinstance(d, str):
        return repr(d)
    k = d[0]
    if k in ("L", "D"):
        return f"({k!r}, {dlit(d[1])})"
    if k == "M":
        return repr(d)
    if k == "Lit":
        return "('Lit', (" + "".join(lit(v) + ", " for v in d[1]) + "))"
    if k == "U":
        disc = "None" if d[2] is None else f"({d[2][0]!r}, {d[2][1]!r})"
        return "('U', (" + "".join(dlit(x) + ", " for x in d[1]) + "), " + disc + ")"
    raise SystemExit(d)


def main():
    import anthropic
    import openai

    versions = {"anthropic": anthropic.__version__, "openai": openai.__version__}
    for pkg, roots in ROOTS.items():
        g = Gen(pkg)
        unions = {}
        for r in roots:
            t = resolve(r)
            if inspect.isclass(t):
                walk_roots(g, t)
            else:
                # a union a stream's events are built as: its descriptor, in _UNIONS
                unions[r.rsplit(".", 1)[1]] = g.desc(t)
        spec = []
        for n, cls, base, fields in g.order:
            fs = []
            for fname, alias, d, dflt in fields:
                if dflt[0] == "v":
                    json.dumps(dflt[1])
                fs.append([fname, alias, d, list(dflt)])
            spec.append([n, cls.__name__.split("[")[0], cls.__module__, base, fs])
        text = json.dumps(spec, separators=(",", ":"))
        out = [
            f"# The response types of {pkg} {versions[pkg]}, written by tools/gen_llm_types.py from the SDK's own models:",
            "# do not edit. Each class: [name, its __name__, module, base, fields]; each field: [name, API name, type",
            "# descriptor, default] (see the tool for the descriptors).",
            "import json as _json",
            "from _brevi_sdk import _build, _tup",
            "",
            "_C = _build(" + repr(text) + ")",
        ]
        for n, _, _, _ in g.order:
            out.append(f"{n} = _C[{n!r}]")
        out.append("")
        out.append("_UNIONS = {}")
        for n, d in unions.items():
            out.append(f"_UNIONS[{n!r}] = _tup(_json.loads({json.dumps(d, separators=(',', ':'))!r}))")
        if pkg == "anthropic":
            # the error classes picked by (status, error.details.error_code): [status, code, __name__, base, module]
            import anthropic._error_classes as ec

            errs = []
            for (st, code), c in ec.ERROR_CLASSES.items():
                if code is not None:
                    errs.append([st, code, c.__name__, c.__mro__[1].__name__, c.__module__])
            out.append("")
            out.append("_ERRORS = " + repr(json.dumps(errs, separators=(",", ":"))))
        out.append("")
        path = f"src/dynpy/py/_brevi_{pkg}_types.py"
        with open(path, "w") as f:
            f.write("\n".join(out))
        print(path, len(g.order), "classes")
        for mod in EXPORTS[pkg]:
            write_exports(g, pkg, mod)


def write_exports(g, pkg, mod):
    # the names a types module of the SDK has: its model classes, and its request param types (TypedDicts and
    # unions of them, which a program only builds dicts with or names in annotations) as dict, literal aliases as str
    m = importlib.import_module(mod)
    names = []
    aliases = []
    for name in sorted(vars(m)):
        if name.startswith("_"):
            continue
        obj = getattr(m, name)
        if inspect.isclass(obj) and obj in g.classes:
            names.append((g.classes[obj], name))
        elif typing.is_typeddict(obj) or name.endswith("Param") or name.endswith("Params"):
            aliases.append(f"{name} = dict")
        elif get_origin(obj) is typing.Literal or (get_origin(obj) in (typing.Union, pytypes.UnionType) and all(a is str or get_origin(a) is typing.Literal for a in get_args(obj))):
            aliases.append(f"{name} = str")
    out = [f"# {mod}, from Brevi's {pkg} copy: written by tools/gen_llm_types.py, do not edit."]
    for i in range(0, len(names), 8):
        part = names[i:i + 8]
        out.append(f"from _brevi_{pkg}_types import " + ", ".join(a if a == b else f"{a} as {b}" for a, b in part))
    out += aliases
    out.append("")
    path = "src/dynpy/py/" + mod.replace(".", "_") + ".py"
    with open(path, "w") as f:
        f.write("\n".join(out))
    print(path, len(names), "classes", len(aliases), "aliases")


EXPORTS = {"anthropic": ["anthropic.types"], "openai": ["openai.types", "openai.types.chat", "openai.types.responses"]}


if __name__ == "__main__":
    sys.exit(main())
