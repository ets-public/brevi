"""Writes src/dynpy/rt_tdict_data.rs: the __dict__ of each builtin type the dynamic build has, read off CPython;
and src/dynpy/cpy_lines.rs: the __firstlineno__ of each class of the CPython modules Brevi's own copies follow.

The dynamic build's vars(T), T.__dict__ and dir() of a builtin type (or of a class built on one) answer from
this table, so they list exactly the names CPython's type dict holds, in its order, with each value's kind
(method_descriptor, slot wrapper, getset or member descriptor, ...). Each type also records what its layout
gives a subclass: whether instances already have a __dict__ and a __weakref__ slot, whether it is variable
sized, and the solid base CPython's best_base() picks by. Run with the CPython the conformance suite pins
(tools/conformance.py PYTHON_VERSION), which this checks:
    python3.13 tools/gen_type_dicts.py
A type Brevi has that is not listed here makes vars(), __dict__ and dir() refuse rather than guess, and so does
a class of Brevi's own module copies (src/dynpy/py/) that cpy_lines.rs does not list.
"""
import ast
import builtins
import collections
import datetime
import decimal
import itertools
import os
import re
import sys
import types

PIN = (3, 13, 13)
if sys.version_info[:3] != PIN:
    sys.exit(f"run this with CPython {'.'.join(map(str, PIN))}, not {sys.version.split()[0]}")

# Brevi's own (module, name) of a class -> the CPython type it stands for. The names are those Brevi's runtime
# gives the class: init_builtin_classes (rt_core.rs), ext_class (rt_lib.rs, module "builtins" with a dotted
# name) and it_class (rt_lib.rs). Only types written in C are listed: a class CPython defines in Python
# (Counter, JSONDecodeError, Fraction) has functions in its dict that Brevi's natives do not reproduce.
def _coroutine_wrapper_type():
    async def f():
        pass
    c = f()
    t = type(c.__await__())
    c.close()
    return t


TYPES = {
    ("builtins", "object"): object,
    ("builtins", "int"): int,
    ("builtins", "bool"): bool,
    ("builtins", "float"): float,
    ("builtins", "str"): str,
    ("builtins", "bytes"): bytes,
    ("builtins", "bytearray"): bytearray,
    ("builtins", "memoryview"): memoryview,
    ("builtins", "list"): list,
    ("builtins", "tuple"): tuple,
    ("builtins", "dict"): dict,
    ("builtins", "set"): set,
    ("builtins", "frozenset"): frozenset,
    ("builtins", "NoneType"): type(None),
    ("builtins", "type"): type,
    ("builtins", "function"): types.FunctionType,
    ("builtins", "range"): range,
    ("builtins", "slice"): slice,
    ("builtins", "ellipsis"): type(...),
    ("builtins", "NotImplementedType"): type(NotImplemented),
    ("builtins", "mappingproxy"): types.MappingProxyType,
    ("builtins", "builtin_function_or_method"): types.BuiltinFunctionType,
    ("builtins", "method"): types.MethodType,
    ("builtins", "cell"): types.CellType,
    ("builtins", "dict_keys"): type({}.keys()),
    ("builtins", "dict_values"): type({}.values()),
    ("builtins", "dict_items"): type({}.items()),
    ("builtins", "deque"): collections.deque,
    ("collections", "defaultdict"): collections.defaultdict,
    ("collections", "OrderedDict"): collections.OrderedDict,
    ("builtins", "collections._tuplegetter"): type(collections.namedtuple("P", "x").x),
    ("builtins", "re.Pattern"): re.Pattern,
    ("builtins", "re.Match"): re.Match,
    ("builtins", "datetime.date"): datetime.date,
    ("builtins", "datetime.datetime"): datetime.datetime,
    ("builtins", "datetime.time"): datetime.time,
    ("builtins", "datetime.timedelta"): datetime.timedelta,
    ("builtins", "datetime.timezone"): datetime.timezone,
    ("builtins", "decimal.Decimal"): decimal.Decimal,
    ("builtins", "list_iterator"): type(iter([])),
    ("builtins", "tuple_iterator"): type(iter(())),
    ("builtins", "str_ascii_iterator"): type(iter("")),
    ("builtins", "str_iterator"): type(iter("€")),
    ("builtins", "range_iterator"): type(iter(range(0))),
    ("builtins", "dict_keyiterator"): type(iter({})),
    ("builtins", "dict_valueiterator"): type(iter({}.values())),
    ("builtins", "dict_itemiterator"): type(iter({}.items())),
    ("builtins", "set_iterator"): type(iter(set())),
    ("builtins", "list_reverseiterator"): type(reversed([])),
    ("builtins", "reversed"): reversed,
    ("builtins", "dict_reversekeyiterator"): type(reversed({})),
    ("builtins", "dict_reversevalueiterator"): type(reversed({}.values())),
    ("builtins", "dict_reverseitemiterator"): type(reversed({}.items())),
    ("builtins", "map"): map,
    ("builtins", "filter"): filter,
    ("builtins", "zip"): zip,
    ("builtins", "enumerate"): enumerate,
    ("builtins", "generator"): types.GeneratorType,
    ("builtins", "coroutine"): types.CoroutineType,
    ("builtins", "coroutine_wrapper"): _coroutine_wrapper_type(),
    ("builtins", "async_generator"): types.AsyncGeneratorType,
    ("builtins", "callable_iterator"): type(iter(int, 1)),
    ("builtins", "bytes_iterator"): type(iter(b"")),
    ("collections", "_deque_iterator"): type(iter(collections.deque())),
    ("builtins", "iterator"): type(iter(collections.UserList())),
}
for n in ("count", "chain", "islice", "repeat", "cycle", "accumulate", "zip_longest", "starmap", "takewhile",
          "dropwhile", "pairwise", "compress", "filterfalse", "groupby", "product", "permutations", "combinations",
          "combinations_with_replacement", "batched"):
    TYPES[("itertools", n)] = getattr(itertools, n)
TYPES[("itertools", "_grouper")] = type(next(itertools.groupby([1]))[1])
TYPES[("itertools", "_tee")] = type(itertools.tee([])[0])
# every builtin exception class
for n in dir(builtins):
    v = getattr(builtins, n)
    if isinstance(v, type) and issubclass(v, BaseException):
        TYPES[("builtins", n)] = v

KINDS = {
    types.MethodDescriptorType: 0,
    types.WrapperDescriptorType: 1,
    types.GetSetDescriptorType: 2,
    types.MemberDescriptorType: 3,
    types.ClassMethodDescriptorType: 4,
    types.BuiltinFunctionType: 5,
}



def lit(s):
    out = []
    for ch in s:
        o = ord(ch)
        if ch == '"':
            out.append('\\"')
        elif ch == "\\":
            out.append("\\\\")
        elif ch == "\n":
            out.append("\\n")
        elif ch == "\t":
            out.append("\\t")
        elif 32 <= o < 127:
            out.append(ch)
        else:
            out.append("\\u{%x}" % o)
    return '"' + "".join(out) + '"'


def entry(t, k, v):
    # (name, kind, text): kinds 0-5 are the descriptor kinds above; 6 None; 7 a str (text); 8 an int (text);
    # 9 a tuple of str (text, comma-separated); 10 a staticmethod of a builtin method of the type
    if k == "__new__" and type(v) is types.BuiltinFunctionType:
        return (k, 5, "")
    kd = KINDS.get(type(v))
    if kd is not None and kd != 5:
        return (k, kd, "")
    if type(v) is staticmethod and type(v.__func__) is types.BuiltinFunctionType:
        return (k, 10, "")
    if v is None:
        return (k, 6, "")
    if type(v) is str:
        return (k, 7, v)
    if type(v) is int:
        return (k, 8, str(v))
    if type(v) is tuple and all(type(x) is str for x in v):
        return (k, 9, ",".join(v))
    return None


def solid(t):
    # CPython's solid_base(): the nearest type in the base chain that adds instance fields beyond __dict__ and
    # __weakref__
    if t is object:
        return object
    b = t.__base__
    size = t.__basicsize__
    if t.__dictoffset__ and not b.__dictoffset__:
        size -= 8
    if t.__weakrefoffset__ and not b.__weakrefoffset__:
        size -= 8
    if size != b.__basicsize__ or t.__itemsize__ != b.__itemsize__:
        return t
    return solid(b)


def key_of(t):
    for k, v in TYPES.items():
        if v is t:
            return k
    return None


rows = []
for (mod, name), t in TYPES.items():
    ents = []
    ok = True
    for k, v in vars(t).items():
        e = entry(t, k, v)
        if e is None:
            print(f"skipped {mod}:{name}: {k} is a {type(v).__name__}", file=sys.stderr)
            ok = False
            break
        ents.append(e)
    if not ok:
        continue
    sb = key_of(solid(t))
    if sb is None:
        print(f"skipped {mod}:{name}: solid base {solid(t)!r} not listed", file=sys.stderr)
        continue
    flags = (1 if t.__dictoffset__ else 0) | (2 if t.__weakrefoffset__ else 0) | (4 if t.__itemsize__ else 0)
    rows.append((mod, name, t.__name__, flags, sb, ents))

out = []
out.append("// Generated by tools/gen_type_dicts.py from CPython %s: do not edit by hand." % ".".join(map(str, PIN)))
out.append("// The names in these dicts and the __doc__ strings are CPython's (PSF License Version 2: src/dynpy/py/LICENSE-PSF.txt).")
out.append("// (Brevi module, Brevi name, CPython __name__, flags 1 dict 2 weakref 4 var-sized, solid base, entries)")
out.append("pub type TEnt = (&'static str, u8, &'static str);")
out.append("pub static TYPE_DICTS: &[(&str, &str, &str, u8, (&str, &str), &[TEnt])] = &[")
for mod, name, pyname, flags, sb, ents in rows:
    es = ", ".join("(%s, %d, %s)" % (lit(k), kd, lit(tx)) for k, kd, tx in ents)
    out.append("    (%s, %s, %s, %d, (%s, %s), &[%s])," % (lit(mod), lit(name), lit(pyname), flags, lit(sb[0]), lit(sb[1]), es))
out.append("];")
path = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "src", "dynpy", "rt_tdict_data.rs")
with open(path, "w") as f:
    f.write("\n".join(out) + "\n")
print(f"wrote {len(rows)} types to src/dynpy/rt_tdict_data.rs")

# Brevi's module copies whose classes are CPython's own, class body for class body (src/dynpy/py/collections.py,
# collections_abc.py, string.py): module -> the CPython file. A class whose dict Brevi's copy cannot match is left out.
COPIES = {"collections.abc": "_collections_abc.py", "collections": "collections/__init__.py", "string": "string.py"}
LEFT_OUT = {
    ("collections.abc", "Callable"): "its __class_getitem__ is _CallableGenericAlias, which Brevi does not have",
}
lib = os.path.dirname(os.__file__)
lines = []
for mod, rel in COPIES.items():
    tree = ast.parse(open(os.path.join(lib, rel), encoding="utf-8").read())

    def walk(body, prefix):
        for node in body:
            if isinstance(node, ast.ClassDef):
                qn = prefix + node.name
                first = min([node.lineno] + [d.lineno for d in node.decorator_list])
                if (mod, qn) not in LEFT_OUT:
                    lines.append((mod, qn, first))
                walk(node.body, qn + ".")
            elif isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef)):
                walk(node.body, prefix + node.name + ".<locals>.")
            elif isinstance(node, (ast.If, ast.Try)):
                walk(node.body, prefix)

    walk(tree.body, "")
out = ["// Generated by tools/gen_type_dicts.py from CPython %s: do not edit by hand." % ".".join(map(str, PIN)),
       "// (module, class __qualname__, __firstlineno__) for the classes of the CPython modules Brevi's own copies follow.",
       "pub static CPY_CLASS_LINES: &[(&str, &str, usize)] = &["]
for mod, qn, first in lines:
    out.append("    (%s, %s, %d)," % (lit(mod), lit(qn), first))
out.append("];")
with open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "src", "dynpy", "cpy_lines.rs"), "w") as f:
    f.write("\n".join(out) + "\n")
print(f"wrote {len(lines)} class lines to src/dynpy/cpy_lines.rs")
