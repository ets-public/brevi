# functools.cached_property and functools.RLock, as CPython's: the value is cached in the instance's __dict__ under
# the name __set_name__ gave, a subclass with __set__ still caches, and the errors match.
from functools import cached_property, RLock
import functools
print(repr(RLock()).split()[0][:6], cached_property.__module__, cached_property.__name__, functools.RLock is RLock)
class C:
    n = 0
    @cached_property
    def cost(self):
        "doc"
        C.n += 1
        return C.n
class S:
    __slots__ = ('a',)
    @cached_property
    def cost(self): return 1
c = C(); print(c.cost, c.cost, C.cost.__doc__, C.cost.__module__, isinstance(C.cost, cached_property), c.__dict__)
try: S().cost
except TypeError as e: print(e)
try:
    class R:
        @cached_property
        def a(self): pass
        b = a
except TypeError as e: print(e)
class ro(cached_property):
    def __set__(self, obj, v): raise AttributeError("ro")
class T:
    def __init__(s,p): s._p=p
    @ro
    def prop(s): return s._p
t=T(1); print(t.prop); t._p=9; print(t.prop)
