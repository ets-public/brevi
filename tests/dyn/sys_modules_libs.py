# sys.modules with keys known only when the program runs, in a program that imports library modules whose
# import loads others: a module the program imports or stores is read back as CPython 3.13 has it.
import sys, json, functools
def mod(name):
    return sys.modules[name]
print(mod("json") is json, mod("functools") is functools, mod("sys") is sys, mod("__main__").mod is mod)
def replaced(name, new):
    old = sys.modules[name]
    sys.modules[name] = new
    try:
        return sys.modules[name] is new, sys.modules.get(name) is new
    finally:
        sys.modules[name] = old
print(replaced("functools", json), sys.modules["functools"] is functools)
key = "stored"
sys.modules[key] = 5
print(key in sys.modules, sys.modules[key], [k in sys.modules for k in ("json", "sys", "os", "stored")])
