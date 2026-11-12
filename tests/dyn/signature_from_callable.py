# inspect.Signature.from_callable on defs, wrappers and bound methods (expected = CPython 3.13)
from inspect import Signature
import functools, inspect
def orig(a, /, b, c=True): ...
lru = functools.lru_cache(1)(orig)
print(str(Signature.from_callable(lru)), str(Signature.from_callable(lru.cache_info)), str(Signature.from_callable(lru.cache_clear)))
@functools.singledispatch
def func(item, arg: int) -> str:
    return str(item)
@func.register
def _(item: int, arg: bytes) -> str:
    return str(item)
print(str(Signature.from_callable(func)))
class A:
    @functools.singledispatchmethod
    def func(self, item, arg: int) -> str:
        return str(item)
    @func.register
    def _(self, item: int, arg: bytes) -> str:
        return str(item)
    @functools.singledispatchmethod
    @classmethod
    def cls_func(cls, item, arg: int) -> str:
        return str(arg)
    @functools.singledispatchmethod
    @staticmethod
    def static_func(item, arg: int) -> str:
        return str(arg)
    def m(self, x, *a, k=3, **kw) -> None: pass
    @classmethod
    def c(cls, y: 'A' = None): pass
    @staticmethod
    def s(*, z): pass
for x in [A.func, A().func, A.cls_func, A.static_func, A.m, A().m, A.c, A().c, A.s]:
    print(str(Signature.from_callable(x)))
L = []
def d(a, b: A = 1, *args: int, c: float = 2.5, d, **e: str) -> 'x': pass
s = Signature.from_callable(d)
L.append(1)
def e(p=L, q=lambda: 0): pass
print(s, repr(s))
print(Signature.from_callable(lambda x, y=2: 0))
print(isinstance(s, Signature), type(s), f"{s}", inspect.Signature is Signature)
def w(*a, **k): pass
print(Signature.from_callable(w), Signature.from_callable(e) == None if False else '')
@functools.wraps(orig)
def wr(*a): pass
print(Signature.from_callable(wr))
