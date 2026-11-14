# sys.modules keyed by a def's parameter, when every call of the def passes a str literal (each read is then
# decided as that literal is), and by a library module the program imports before anything else runs: as
# CPython 3.13 answers, even though the program imports modules whose import loads others.
import sys, contextlib, functools, json
@contextlib.contextmanager
def replaced_module(name, replacement):
    original_module = sys.modules[name]
    sys.modules[name] = replacement
    try:
        yield
    finally:
        sys.modules[name] = original_module
def has(name, default="json"):
    return name in sys.modules, default in sys.modules, sys.modules.get(default) is json
with replaced_module("functools", json):
    print(sys.modules["functools"] is json, functools.reduce(lambda a, b: a + b, [1, 2, 3]))
print(sys.modules["functools"] is functools, "json" in sys.modules, sys.modules.get("functools") is functools)
print(has("sys"), has("functools", default="functools"), has(name="json"))
