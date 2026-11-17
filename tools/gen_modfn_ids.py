"""Writes src/dynpy/rt_modfn_data.rs: what CPython says each function of a natively built stdlib module is.

A dynamic build makes every function of those modules (json.dumps, math.sqrt, dataclasses.dataclass) as one
Brevi builtin (modfn in rt_mod.rs). In CPython some are Python functions (type `function`, json.dumps) and some
are C functions (`builtin_function_or_method`, math.sqrt), with their own __module__ ('posixpath', '_heapq'),
__name__ (bisect.bisect is bisect_right), __qualname__ and __doc__. modfn reads this table so that type(),
repr() and those attributes answer as CPython's; a name the table does not list as a function of its module,
in a module it covers, has its introspection refused rather than guessed. Run with the CPython the
conformance suite pins (tools/conformance.py PYTHON_VERSION), which this checks:
    python3.13 tools/gen_modfn_ids.py
"""
import importlib
import os
import sys
import types

PIN = (3, 13, 13)
if sys.version_info[:3] != PIN:
    sys.exit(f"run this with CPython {'.'.join(map(str, PIN))}, not {sys.version.split()[0]}")

# the stdlib modules whose functions the dynamic build makes with modfn (module_call in rt_mod.rs and
# ext_module_call / ext_module_call2 behind it); Brevi's own _brevi_* shims and third-party modules stay out
MODULES = [
    "atexit", "base64", "binascii", "bisect", "collections", "copy", "csv", "dataclasses", "datetime",
    "decimal", "enum", "fractions", "functools", "gc", "hashlib", "heapq", "html", "importlib.metadata",
    "inspect", "itertools", "json", "json.decoder", "json.encoder", "math", "operator", "re", "re._compiler",
    "re._constants", "statistics", "sys", "threading", "time", "weakref", "zoneinfo",
    # reached through modfn too, for a few functions each
    "_asyncio", "_queue", "abc", "contextlib", "fileinput", "graphlib", "mailbox", "queue", "string", "tempfile",
]
# Brevi's own Python copies of stdlib modules (src/dynpy/py, sandbox_module in mod.rs) whose defs the table
# covers by their qualified names: module name as the dynamic build names it, then the copy's file. ntpath is
# left out (on Windows CPython swaps some of its functions for C ones of nt), so are third-party libraries'
# copies and Brevi's _brevi_* modules: CPython has no answer for them, and their defs' introspection is refused.
COPIES = [
    ("random", "random.py"), ("collections", "collections.py"), ("collections.abc", "collections_abc.py"),
    ("os", "os.py"), ("posixpath", "posixpath.py"), ("pathlib", "pathlib.py"), ("io", "io.py"),
    ("textwrap", "textwrap.py"), ("logging", "logging.py"), ("argparse", "argparse.py"), ("string", "string.py"),
    ("_string", "_string.py"), ("asyncio", "asyncio.py"), ("asyncio.subprocess", "asyncio_subprocess.py"),
    ("contextlib", "contextlib.py"), ("sqlite3", "sqlite3.py"),
]


def rust_str(s):
    out = []
    for ch in s:
        o = ord(ch)
        if ch == "\\":
            out.append("\\\\")
        elif ch == "\"":
            out.append("\\\"")
        elif ch == "\n":
            out.append("\\n")
        elif ch == "\t":
            out.append("\\t")
        elif 32 <= o < 127:
            out.append(ch)
        else:
            out.append("\\u{%x}" % o)
    return "\"" + "".join(out) + "\""


def opt(s):
    return "None" if s is None else "Some(" + rust_str(s) + ")"


found = []
for m in MODULES:
    mod = importlib.import_module(m)
    for n in sorted(vars(mod)):
        f = vars(mod)[n]
        if isinstance(f, types.FunctionType):
            kind = 0
        elif isinstance(f, types.BuiltinFunctionType) and isinstance(f.__self__, types.ModuleType):
            kind = 1
        else:
            continue
        found.append((m, n, kind, f))
# One object under several names (bisect.bisect is bisect_right, statistics.sqrt is math.sqrt, operator.__add__
# is add): each row names the row Brevi makes the one function from, so `is` holds as in CPython. That is the
# name in the function's own module when the table covers it, else the first row whose name is its __name__.
def canon(f):
    same = [i for i, r in enumerate(found) if r[3] is f]
    for test in (lambda r: r[0] == f.__module__ and r[1] == f.__name__, lambda r: r[1] == f.__name__, lambda r: True):
        for i in same:
            if test(found[i]):
                return i
rows = []
for m, n, kind, f in found:
    rows.append(f"    ({rust_str(m)}, {rust_str(n)}, {kind}, {opt(f.__module__)}, {rust_str(f.__name__)}, {rust_str(f.__qualname__)}, {opt(f.__doc__)}, {canon(f)}),")

import ast
import inspect

MISSING = object()
here = os.path.dirname(os.path.abspath(__file__))


def copy_defs(src):
    # (qualname, decorator names) of each def at module level or in a class body (not inside a def: those are
    # nested functions, which the runtime refuses to introspect without looking here)
    out = []

    def walk(body, prefix):
        for st in body:
            if isinstance(st, (ast.FunctionDef, ast.AsyncFunctionDef)):
                out.append((prefix + st.name, [ast.unparse(d) for d in st.decorator_list]))
            elif isinstance(st, ast.ClassDef):
                walk(st.body, prefix + st.name + ".")
            elif isinstance(st, (ast.If, ast.Try, ast.With, ast.For, ast.While)):
                for f in ("body", "orelse", "finalbody"):
                    walk(getattr(st, f, []), prefix)
                for h in getattr(st, "handlers", []):
                    walk(h.body, prefix)
    walk(ast.parse(src).body, "")
    return out


def cpython_def(mod, qual, decs):
    # CPython's function at the qualified name the copy's def has; None when it is not a plain Python function
    # (a C function or method, a name CPython lacks, a decorator whose result is not the def)
    obj = mod
    for part in qual.split("."):
        obj = inspect.getattr_static(obj, part, MISSING)
        if obj is MISSING:
            return None
    plain = [d for d in decs if d not in ("staticmethod", "classmethod", "abstractmethod", "abc.abstractmethod")]
    if isinstance(obj, (staticmethod, classmethod)):
        obj = obj.__func__
    elif isinstance(obj, property):
        if len(plain) != 1 or not (plain[0] == "property" or plain[0].endswith((".setter", ".getter", ".deleter"))):
            return None
        side = plain[0].rsplit(".", 1)[-1]
        obj = getattr(obj, {"property": "fget", "getter": "fget", "setter": "fset", "deleter": "fdel"}[side])
        plain = []
    if plain or not isinstance(obj, types.FunctionType) or vars(obj):
        return None
    return obj


copy_rows = []
for m, fname in COPIES:
    mod = importlib.import_module(m)
    with open(os.path.join(here, "..", "src", "dynpy", "py", fname), encoding="utf-8") as fh:
        for qual, decs in copy_defs(fh.read()):
            f = cpython_def(mod, qual, decs)
            copy_rows.append((m, qual, f))
copy_rows = sorted({(m, q): f for m, q, f in copy_rows}.items(), key=lambda r: (r[0][0].encode(), r[0][1].encode()))

path = os.path.join(here, "..", "src", "dynpy", "rt_modfn_data.rs")
with open(path, "w") as fh:
    fh.write("// Generated by tools/gen_modfn_ids.py from CPython 3.13.13: do not edit by hand.\n")
    fh.write("// The names and __doc__ strings are CPython's (PSF License Version 2: src/dynpy/py/LICENSE-PSF.txt).\n")
    fh.write("// The modules whose functions this table covers, then (module, name, kind 0 Python function 1 C function,\n")
    fh.write("// __module__, __name__, __qualname__, __doc__, the row of the one object it is), sorted by module and name.\n")
    fh.write("pub static MODFN_MODULES: &[&str] = &[" + ", ".join(rust_str(m) for m in MODULES) + "];\n")
    fh.write("#[allow(clippy::type_complexity)]\n")
    fh.write("pub static MODFN_IDS: &[(&str, &str, u8, Option<&str>, &str, &str, Option<&str>, usize)] = &[\n")
    fh.write("\n".join(rows) + "\n];\n")
    fh.write("// Each def of Brevi's own copies of stdlib modules, by (module, qualified name), sorted: CPython's function\n")
    fh.write("// there as (__module__, __name__, __qualname__, __doc__), or None when CPython's is not a plain Python function.\n")
    fh.write("#[allow(clippy::type_complexity)]\n")
    fh.write("pub static COPYFN_IDS: &[(&str, &str, Option<(Option<&str>, &str, &str, Option<&str>)>)] = &[\n")
    for (m, q), f in copy_rows:
        who = "None" if f is None else f"Some(({opt(f.__module__)}, {rust_str(f.__name__)}, {rust_str(f.__qualname__)}, {opt(f.__doc__)}))"
        fh.write(f"    ({rust_str(m)}, {rust_str(q)}, {who}),\n")
    fh.write("];\n")
print(f"wrote {len(rows)} functions of {len(MODULES)} modules and {len(copy_rows)} defs of {len(COPIES)} copies to src/dynpy/rt_modfn_data.rs")
