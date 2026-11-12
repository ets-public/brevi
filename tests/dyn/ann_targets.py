# Annotated assignments as CPython 3.13 compiles them: a name in parentheses with no value binds nothing, and a
# target with no value evaluates only its object (and index), never the attribute or item itself.
x = 10
def f():
    (x): int
    return x
print(f())
def g():
    (y): int = 3
    return y
print(g())
class O:
    def __getattr__(self, n):
        print("get", n)
        raise AttributeError(n)
class L:
    def __getitem__(self, k):
        print("getitem", k)
        return 0
def mk(tag):
    print("mk", tag)
    return O()
def h():
    mk("h").a: int
    l = L()
    l[print("idx") or 1]: int
    try:
        undefined_name.q: int
    except NameError as e:
        print("NameError", e)
    try:
        z[0]: int
    except NameError as e:
        print("NameError", e)
h()
mk("top").b: int
L()[2]: int
class C:
    (p): int
    (q): int = 5
    r: int
    mk("c").a: int
print(C.__annotations__, C.q, hasattr(C, "p"))
