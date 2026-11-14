# `import pkg.sub` runs the package, then its submodule, sets the submodule on the package and binds the
# package; sys.modules['__main__'] is the script's module. As CPython 3.13.
import sys
print(sys.modules["__main__"] is sys.modules[__name__], sys.modules["__main__"].__name__)
import subpk
print(hasattr(subpk, "one"), hasattr(subpk, "two"))
try:
    subpk.one
except AttributeError as e:
    print(e)
m = subpk
try:
    m.two
except AttributeError as e:
    print(e)
import subpk.one
print(subpk.one.B, hasattr(subpk, "one"), getattr(m, "one") is subpk.one, sys.modules["subpk.one"] is subpk.one)
from subpk import two
print(two.C, subpk.two is two, m.two.C)
import subpk.inner.deep
print(subpk.inner.deep.D, [k in sys.modules for k in ("subpk", "subpk.one", "subpk.two", "subpk.inner", "subpk.inner.deep", "subpk.three")])
import subpk.inner.deep as dd
print(dd is subpk.inner.deep, dd.__name__)
from subpk import one, inner
print(one.B, inner.deep.D)
import circ
print(circ.part.P)
x = 10
def f():
    return 7
main = sys.modules["__main__"]
print(main.x, main.f(), main.f is f, repr(main) == "<module '__main__' from " + repr(__file__) + ">")
try:
    main.later
except AttributeError as e:
    print(e)
later = 1
print(main.later, hasattr(main, "nope"))
main.x = 3
print(x)
