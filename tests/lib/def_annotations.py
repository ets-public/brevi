# A def's __annotations__ (evaluated when the def is made, as in CPython 3.13), kept while the program reads
# __annotations__; functools.wraps/update_wrapper copy them, and the other attributes CPython's copy.
import functools
def f(a: int, *args: str, b: 'x' = 2, **kw: float) -> None:
    "doc"
def g(): pass
print(f.__annotations__, g.__annotations__, g.__annotations__ is g.__annotations__)
class K:
    def m(self, x: "K") -> int: return 1
print(K.m.__annotations__)
@functools.wraps(f)
def w(*a, **k): pass
print(w.__annotations__ is f.__annotations__, w.__name__, w.__doc__)
def wrapper(b: 'prior'): pass
functools.update_wrapper(wrapper, f)
print(wrapper.__annotations__)
import functools
class C2:
    x: int = 1
    def m(self, a: int) -> str: return ""
print(C2.__annotations__, C2.m.__annotations__, C2().m.__annotations__)
f = lambda: 0
try: print(f.__annotations__)
except Exception as e: print(type(e).__name__)
print(max.__doc__[:10], functools.WRAPPER_ASSIGNMENTS)
import contextlib
@contextlib.contextmanager
def cm(cls, arg: int) -> str:
    yield 1
print(cm.__annotations__)
c = classmethod(cm)
print(c.__annotations__)
class W:
    @functools.singledispatchmethod
    @classmethod
    def m(cls, arg: int) -> str:
        "doc"
        return str(arg)
print(W.m.__annotations__, W().m.__annotations__)
print(cm.__qualname__, cm.__module__, cm.__wrapped__.__name__)
# class C:
#     @classmethod
class S:
    @classmethod
    def c(cls): return 1
    @staticmethod
    def s(): return 2
print(S.__dict__["c"].__func__.__name__, S.__dict__["s"].__func__())
def tw(*a): pass
functools.update_wrapper(tw, type)
print(tw.__name__, tw.__annotations__, tw.__type_params__, int.__type_params__)
def bw(): pass
functools.update_wrapper(bw, max)
print(bw.__name__, bw.__doc__ == max.__doc__, max.__doc__.splitlines()[0], bw.__annotations__)
import math
print(math.pow.__doc__ is None or math.pow.__doc__ != pow.__doc__)
