# sys.modules through `import sys` in a function, and through a name that is not sys: as CPython 3.13.
def f():
    import sys
    return "os" in sys.modules, "nosuch" not in sys.modules, sys.modules["sys"] is sys, sys.modules.get("nosuch", 3)
print(f())
def g():
    import sys as s
    s.modules["mine"] = 1
    return s.modules["mine"], "mine" in s.modules
print(g())
class Holder:
    modules = {"a": 1}
def h(sys):
    return sys.modules["a"], "a" in sys.modules, sys.modules.get("b", 2)
print(h(Holder()))
