# functools.partialmethod, as CPython 3.13's: bound through the wrapped descriptor's __get__ when it makes
# something new (classmethod, staticmethod, a partialmethod's own methods), else as an instance method.
import functools


def capture(*args, **kw):
    return args, kw


class A:
    nothing = functools.partialmethod(capture)
    positional = functools.partialmethod(capture, 1)
    keywords = functools.partialmethod(capture, a=2)
    both = functools.partialmethod(capture, 3, b=4)
    nested = functools.partialmethod(positional, 5)
    static = functools.partialmethod(staticmethod(capture), 8)
    cls = functools.partialmethod(classmethod(capture), d=9)


a = A()
print(a.nothing() == ((a,), {}), a.positional(0) == ((a, 1, 0), {}))
print(a.keywords(b=1) == ((a,), {"a": 2, "b": 1}), a.both(5, c=6) == ((a, 3, 5), {"b": 4, "c": 6}))
print(a.nested(6) == ((a, 1, 5, 6), {}))
print(a.static(1) == ((8, 1), {}), A.static(2) == ((8, 2), {}))
print(a.cls(1) == ((A, 1), {"d": 9}), A.cls(e=1) == ((A,), {"d": 9, "e": 1}))
print(A.nothing(1) == ((1,), {}))
print(type(A.__dict__["both"]).__name__, isinstance(A.__dict__["both"], functools.partialmethod))
print(A.__dict__["nested"].args, A.__dict__["nested"].func is capture)
print(repr(A.__dict__["keywords"]).startswith("functools.partialmethod(<function capture at "), repr(A.__dict__["keywords"]).endswith(", a=2)"))
print(A.positional.__partialmethod__ is A.__dict__["positional"])
for bad in ((None, 1), ()):
    try:
        functools.partialmethod(*bad)
    except TypeError as e:
        print("TypeError", type(e).__name__)
