# A def's __defaults__ and __kwdefaults__ are CPython's: the tuple of positional defaults and the dict of
# keyword-only ones (None when there are none), the same objects the calls use, through methods, classmethods,
# closures and lambdas. A staticmethod or classmethod object itself has neither.
import functools


def h(a=1, b="x"):
    return a, b


def g():
    pass


def k(a, *, b=2, c):
    pass


def p(a, /, b=[1], *args, c=None, **kw):
    b.append(len(b))
    return b


lam = lambda x, y=3: x
print(h.__defaults__, g.__defaults__, k.__defaults__, k.__kwdefaults__, p.__defaults__, p.__kwdefaults__, lam.__defaults__)
print(g.__kwdefaults__, h.__kwdefaults__)
print(p(0) is p.__defaults__[0], p.__defaults__)


class C:
    def m(self, z=5):
        return z

    @staticmethod
    def s(q=7):
        return q

    @classmethod
    def c(cls, r=8):
        return r


print(C.m.__defaults__, C().m.__defaults__, C.s.__defaults__, C().c.__defaults__, C.c.__defaults__)
f = C.__dict__["c"].__func__
print(C().c.__func__ is f, C.c.__func__ is f, C.c.__name__, C.c.__qualname__)
for d in (C.__dict__["s"], C.__dict__["c"]):
    try:
        d.__defaults__
    except AttributeError as e:
        print("AttributeError:", e)


def outer():
    v = 10

    def inner(w=v):
        return w
    return inner


print(outer().__defaults__)


@functools.wraps(h)
def w(*a):
    pass


print(w.__defaults__, getattr(h, "__defaults__"), getattr(k, "__kwdefaults__"))
try:
    len.__defaults__
except AttributeError as e:
    print("AttributeError:", e)
