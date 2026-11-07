# Writes src/dynpy/py/_brevi_mcp_types.py, the tables of Brevi's mcp copy, from the SDK's own pydantic models:
#   - the protocol types programs see (mcp.types / mcp_types, the "monolith"): each class with its fields in the
#     SDK's order, the wire name of each (alias), a type descriptor and the default;
#   - the wire shapes the SDK checks a result against before it is sent (mcp_types._v2025_11_25 for the
#     handshake-era versions, _v2026_07_28 for the modern one), with whether each keeps unknown keys, so a
#     result is dumped with the same fields in the same order;
#   - which (method, protocol version) pairs exist, and the wire type of each request, result and notification.
#
#   python3 tools/gen_mcp_types.py      (needs mcp 2.3.0 / mcp-types 2.3.0 installed)
#
# Descriptors are those of tools/gen_llm_types.py: None any value; "str" "int" "float" "bool" "none" "bytes";
# ("L", d) list; ("D", d) dict values; ("Lit", values); ("M", "Class"); ("U", (d, ...), disc).
import inspect
import json
import os
import sys
import typing

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import pydantic  # noqa: E402
from typing_extensions import is_typeddict  # noqa: E402

from gen_llm_types import Gen  # noqa: E402


class G(Gen):
    def desc(self, t, meta=()):
        if is_typeddict(t):
            return ("D", None)
        if t is bytes:
            return "bytes"
        if getattr(t, "__name__", None) == "JSONValue" or type(t).__name__ == "TypeAliasType" and t.__name__ == "JSONValue":
            return None
        if inspect.isclass(t) and t.__module__.startswith("pydantic.networks"):
            return "str"
        return super().desc(t, meta)


def table(g):
    spec = []
    for n, cls, base, fields in g.order:
        fs = []
        for fname, alias, d, dflt in fields:
            if dflt[0] == "v":
                json.dumps(dflt[1])
            fs.append([fname, alias, d, list(dflt)])
        extra = cls.model_config.get("extra") or "ignore"
        spec.append([n, cls.__name__.split("[")[0], base, extra, fs])
    return json.dumps(spec, separators=(",", ":"))


def pyd(d):
    # a descriptor of tools/gen_llm_types.py as the source of a descriptor of Brevi's pydantic (src/dynpy/py/pydantic.py)
    if d is None:
        return "('any', 'Any')"
    if isinstance(d, str):
        return "(%r,)" % d
    k = d[0]
    if k == "L":
        if d[1] is None:
            return "('list', ('any', 'Any'), 'list', True)"
        return "('list', %s, 'list', False)" % pyd(d[1])
    if k == "D":
        return "('dict', ('str',), %s, 'dict', False)" % pyd(d[1])
    if k == "Lit":
        return "('literal', (%s))" % "".join(repr(v) + ", " for v in d[1])
    if k == "M":
        return "('lazy', lambda: %s, %r)" % (d[1], d[1])
    if k == "U":
        return "('union', (%s))" % "".join(pyd(x) + ", " for x in d[1])
    raise SystemExit(d)


def mono_source(g, all_names):
    # each class of the monolith as a model of Brevi's pydantic: its fields with their wire names and defaults
    out = []
    names = {n for n, _, _, _ in g.order}
    for n, cls, base, fields in g.order:
        b = base if base in names else "_MCPModel"
        cfg = {"populate_by_name": True}
        if cls.model_config.get("extra"):
            cfg["extra"] = cls.model_config["extra"]
        out.append("class %s(%s):" % (n, b))
        out.append("    model_config = %r" % cfg)
        anns = []
        for fname, alias, d, dflt in fields:
            args = []
            if dflt[0] == "v":
                args.append(repr(dflt[1]))
            elif dflt[0] == "f":
                args.append("default_factory=%s" % dflt[1])
            if alias is not None and alias != fname:
                args.append("alias=%r" % alias)
            out.append("    %s = _Field(%s)" % (fname, ", ".join(args)))
            anns.append("(%r, %s)" % (fname, pyd(d)))
        out.append("    __annotations__ = (%s)" % "".join(a + ", " for a in anns))
        if cls.__name__ != n:
            out.append("%s.__name__ = %r" % (n, cls.__name__))
        out.append("%s.__module__ = 'mcp_types'" % n)
        out.append("")
    return out


def main():
    import mcp
    import mcp_types._types as T
    import mcp_types._v2025_11_25 as v25
    import mcp_types._v2026_07_28 as v26
    import mcp_types.methods as M
    from importlib.metadata import version

    mono = G("mcp_types")
    for n, c in vars(T).items():
        if inspect.isclass(c) and issubclass(c, pydantic.BaseModel) and c.__module__ == "mcp_types._types":
            mono.model(c)
    import mcp_types.jsonrpc as J
    for n in J.__all__:
        c = getattr(J, n)
        if inspect.isclass(c) and issubclass(c, pydantic.BaseModel):
            mono.model(c)
    gens = {"h": G("mcp_types"), "m": G("mcp_types")}
    era = {v25: "h", v26: "m"}
    surf = {}
    for key, tab in [("results", M.SERVER_RESULTS), ("notifications", M.SERVER_NOTIFICATIONS), ("server_requests", M.SERVER_REQUESTS),
                     ("requests", M.CLIENT_REQUESTS), ("client_notifications", M.CLIENT_NOTIFICATIONS), ("client_results", M.CLIENT_RESULTS)]:
        out = {}
        for (method, ver), c in tab.items():
            arms = [c] if inspect.isclass(c) else [a for a in typing.get_args(c)]
            mod = sys.modules[arms[0].__module__]
            e = era[mod]
            names = [gens[e].model(a) for a in arms]
            out.setdefault(method, {})[ver] = names if len(names) > 1 else names[0]
        surf[key] = out
    import mcp_types as MT
    unions = []
    for n in MT.__all__:
        v = getattr(MT, n)
        if type(v).__name__ in ("UnionType", "_UnionGenericAlias") and all(inspect.isclass(a) for a in typing.get_args(v)):
            arms = [a for a in typing.get_args(v)]
            if all(issubclass(a, pydantic.BaseModel) for a in arms):
                unions.append("%s = (%s)" % (n, "".join(mono.classes[a] + ", " for a in arms)))
    src = [
        "# The protocol types of mcp %s (mcp-types %s) as models of Brevi's pydantic, written by tools/gen_mcp_types.py" % (version("mcp"), version("mcp-types")),
        "# from the SDK's own models: do not edit.",
        "from pydantic import BaseModel as _BaseModel, Field as _Field",
        "",
        "",
        "class _MCPModel(_BaseModel):",
        "    model_config = {'populate_by_name': True}",
        "",
        "",
    ] + mono_source(mono, None) + unions + [""]
    with open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "src", "dynpy", "py", "_brevi_mcp_models.py"), "w") as f:
        f.write("\n".join(src))
    lines = [
        f"# The protocol types and wire shapes of mcp {version('mcp')} (mcp-types {version('mcp-types')}), written by",
        "# tools/gen_mcp_types.py from the SDK's own models: do not edit. Each class: [name, __name__, base, extra, fields];",
        "# each field: [name, wire name, type descriptor, default]. See the tool for the descriptors.",
        "MONO = " + repr(table(mono)),
        "WIRE_H = " + repr(table(gens["h"])),
        "WIRE_M = " + repr(table(gens["m"])),
        "SURFACE = " + repr(json.dumps(surf, separators=(",", ":"), sort_keys=True)),
        "",
    ]
    path = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "src", "dynpy", "py", "_brevi_mcp_types.py")
    with open(path, "w") as f:
        f.write("\n".join(lines))
    print(path, len(mono.order), len(gens["h"].order), len(gens["m"].order))


main()
