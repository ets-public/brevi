# isinstance() and class patterns with typing.Protocol classes (rt_typing.rs proto_instancecheck): CPython's
# _ProtocolMeta rules, the TypeErrors, and a protocol that cannot be instantiated
from typing import Protocol, runtime_checkable, TypeVar, Generic
T = TypeVar("T")

@runtime_checkable
class P(Protocol):
    x: int
    y: int

class A:
    def __init__(self, x, y):
        self.x = x
        self.y = y

class B(A): ...

for cls in (A, B):
    inst = cls(1, 2)
    match inst:
        case P() as p:
            print(isinstance(p, cls), p.x, p.y)
    match inst:
        case P(x=x, y=y):
            print(x, y)
print(isinstance(1, P), isinstance(A(None, None), P))

@runtime_checkable
class M(Protocol):
    def meth(self): ...
    size: int

class C:
    size = 3
    def meth(self): return 1
c = C()
print(isinstance(c, M))
c.meth = None
print(isinstance(c, M))
class D:
    meth = None
    size = 1
print(isinstance(D(), M))
class E: pass
e = E(); e.meth = lambda: 1; e.size = None
print(isinstance(e, M))

class Q(Protocol):
    def f(self): ...
try:
    isinstance(1, Q)
except TypeError as ex:
    print("TE", ex)
try:
    match 1:
        case Q(): pass
except TypeError as ex:
    print("TE", ex)
class Base: pass
try:
    class Bad(Base, Protocol): pass
except TypeError as ex:
    print("TE", ex)
try:
    @runtime_checkable
    class NotP(Base): pass
except TypeError as ex:
    print("TE", ex)

@runtime_checkable
class SizedP(Protocol):
    def __len__(self): ...
print(isinstance([1], SizedP), isinstance(3, SizedP), isinstance("ab", SizedP), isinstance({}, SizedP))

@runtime_checkable
class Child(M, Protocol):
    def g(self): ...
class F(C):
    def g(self): pass
print(isinstance(F(), Child), isinstance(C(), Child))
class Concrete(M):
    size = 0
print(isinstance(Concrete(), M), isinstance(Concrete(), Concrete))
@runtime_checkable
class P(Protocol[T]):
    def get(self) -> T: ...
class Box(Generic[T]):
    def __init__(self, v): self.v = v
    def get(self): return self.v
print(isinstance(Box(1), P), isinstance(3, P))
try:
    P()
except TypeError as e:
    print("TE", e)
@runtime_checkable
class H(Protocol):
    def __hash__(self): ...
print(isinstance([], H), isinstance((), H), isinstance(1, H))
class Eq:
    def __eq__(self, o): return True
print(isinstance(Eq(), H))
@runtime_checkable
class WithEq(Protocol):
    def __eq__(self, o): ...
print(isinstance(1, WithEq), isinstance(Eq(), WithEq))
class Slots:
    __slots__ = ("get",)
s = Slots()
s.get = 5
print(isinstance(s, P))
print(isinstance(Slots(), P))
class Prop:
    @property
    def get(self): raise RuntimeError("no")
print(isinstance(Prop(), P))

