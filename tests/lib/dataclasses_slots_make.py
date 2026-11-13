# dataclasses: abstract recount, frozen subclasses, slots and weakref_slot, make_dataclass (test_dataclasses)
from dataclasses import make_dataclass, field, fields, InitVar, dataclass
C = make_dataclass('C', [('x', int), ('y', int, field(default=5))], namespace={'add_one': lambda self: self.x + 1})
c = C(10); print(c, c.add_one(), C.__module__, C.__qualname__)
ns = {}
make_dataclass('C', [('x', int)], namespace=ns); print(ns)
P = make_dataclass('Point', ['x', ('y', int), 'z'])
print(vars(P(1,2,3)), P.__annotations__)
def post_init(self, y): self.x *= y
D = make_dataclass('D', [('x', int), ('y', InitVar[int])], namespace={'__post_init__': post_init})
print(vars(D(2, 3)), len(fields(D)))
for bad in [(), (1,2,3,4), float, lambda x: x]:
    try: make_dataclass('C', ['a', bad])
    except TypeError as e: print(e)
for f in ['a', 'ab', 'for', '()', '']:
    try: make_dataclass('C', [f, 'a', f])
    except TypeError as e: print(e)
for cn in ['()', 'x,y', '']:
    print(repr(make_dataclass(cn, ['a', 'b']).__name__))
@dataclass
class B1:
    x: int
class B2: pass
E = make_dataclass('E', [('y', int)], bases=(B1, B2))
try: E(2)
except TypeError as e: print(e)
print(E(1, 2), isinstance(E(1,2), B2))
W = make_dataclass('W', [('a', int)], module='custom'); print(W.__module__, W(1).__module__)
S = make_dataclass('S', [('a', int)], slots=True, weakref_slot=True); print(S.__slots__)
try: make_dataclass('B', [('a', int)], weakref_slot=True)
except TypeError as e: print(e)
try: make_dataclass('C', [], xxinit=False)
except TypeError as e: print(e)
M = make_dataclass('M', ['a', 'b']); print(M.__match_args__, M(1, 2))

import abc, inspect
from dataclasses import dataclass, field, MISSING
class Ordered(abc.ABC):
    @abc.abstractmethod
    def __lt__(self, other): pass
    @abc.abstractmethod
    def __le__(self, other): pass
@dataclass(order=True)
class Date(Ordered):
    year: int
    month: int
print(sorted(Date.__abstractmethods__), inspect.isabstract(Date))
print(Date(1,2) < Date(1,3))
class A(abc.ABC):
    @abc.abstractmethod
    def foo(self): pass
@dataclass
class B(A):
    x: int
print(sorted(B.__abstractmethods__), inspect.isabstract(B))
try: B(1)
except TypeError as e: print(e)
@dataclass
class F:
    x: list = field(default=MISSING, default_factory=list)
print(F())
class P:
    pass
print(P.__hash__ is object.__hash__, getattr(P, '__hash__') is object.__hash__)
@dataclass
class Q: x: int
try:
    a, b = Q(1)
except TypeError as e: print(e)
@dataclass(frozen=True)
class Fr:
    x: int
class Sub(Fr):
    def __setattr__(self, n, v):
        super(Sub, self).__setattr__(n, v)
    def __delattr__(self, n):
        super(Sub, self).__delattr__(n)
s = Sub(1)
try: s.x = 2
except Exception as e: print(type(e).__name__, e)
try: del s.x
except Exception as e: print(type(e).__name__, e)

import weakref
from dataclasses import dataclass, field
@dataclass(slots=True, weakref_slot=True)
class A:
    a: int
a = A(1)
r = weakref.ref(a)
print(r() is a, a.__weakref__ is r)
@dataclass(slots=True)
class B:
    a: int
try: weakref.ref(B(1))
except TypeError as e: print(e)
class Root4:
    __slots__ = ('x',)
@dataclass(slots=True)
class C(Root4):
    y: int
    x: int = 3
print(C(1))
@dataclass(slots=True)
class D:
    x: int
try: D()
except TypeError as e: print(e)
print(D(2))

import weakref
class P: pass
p = P()
print(p.__weakref__)
r = weakref.ref(p); print(r is weakref.ref(p), p.__weakref__ is r)
class I(int): pass
try: weakref.ref(I(1))
except TypeError as e: print(e)
try: weakref.ref(ValueError())
except TypeError as e: print(e)
class S:
    __slots__ = ('a',)
try: S().__weakref__
except AttributeError as e: print(e)
class S2:
    __slots__ = ('a', '__weakref__')
s = S2(); print(weakref.ref(s)() is s)
a, b = 1, 2
try:
    a, b = 5
except TypeError as e: print(e)
try:
    a, *b = None
except TypeError as e: print(e)
