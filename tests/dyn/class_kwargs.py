# Class statement keywords on the dynamic build (dynpy/emit.rs class, rt_call.rs make_class_impl): they go to
# __init_subclass__, to a metaclass's __prepare__/__new__/__init__, and an Enum's boundary= sets a Flag's _boundary_
# (STRICT, CONFORM, EJECT, KEEP as enum.py's Flag._missing_ reads it).
class A: pass
try:
    class B(A, x=1): pass
except TypeError as e: print(e)
class P:
    def __init_subclass__(cls, **kw):
        print("isc", cls.__name__, kw)
        super().__init_subclass__()
class Q(P, a=1, b=2): pass
try:
    class R(P, a=1):
        pass
    class S(int, y=2): pass
except TypeError as e: print(e)
class M(type):
    def __new__(m, n, b, ns, **kw):
        print("new", n, kw)
        return super().__new__(m, n, b, ns)
    def __init__(c, n, b, ns, **kw):
        print("init", n, kw)
    @classmethod
    def __prepare__(m, n, b, **kw):
        print("prep", n, kw)
        return {}
class T(metaclass=M, z=3): pass
import enum
print(enum.Flag._boundary_, enum.IntFlag._boundary_, enum.Enum.__dict__.get("_boundary_"))
class E(enum.Enum, boundary=enum.KEEP):
    A = 1
print(hasattr(E, "_boundary_"))
try:
    class F(enum.Flag, boundary=5):
        A = 1
    F(7)
except Exception as e: print(type(e).__name__, e)

import enum

from enum import Flag, IntFlag, STRICT, CONFORM, EJECT, KEEP
class A(IntFlag):
    X = 1
    Y = 2
for v in (-8, -4, -1, 5, 9, -9):
    r = A(v)
    print(v, repr(r), r.value)
class C(Flag, boundary=CONFORM):
    X = 1
    Y = 2
    Z = 8
print(repr(C(7)), repr(C(-1)), repr(~C.X), repr(C(16)), 7 in C, 16 in C)
class J(Flag, boundary=EJECT):
    X = 1
    Y = 4
print(repr(J(3)), repr(~J.X), 3 in J, 5 in J, repr(J(5)))
class JI(IntFlag, boundary=EJECT):
    X = 1
    Y = 4
print(repr(JI.X | 2), repr(JI(2)), type(JI(2)).__name__, repr(~JI.X))
class SI(IntFlag, boundary=STRICT):
    X = 1
    Y = 2
try:
    SI.X | 4
except ValueError as e:
    print("VE", e)
class Base(Flag, boundary=CONFORM):
    pass
class Sub(Base):
    P = 1
print(Sub._boundary_, repr(Sub(3)))
class K(Flag, boundary=KEEP):
    A = 1
    B = 4
print(repr(K(7)), repr(~K.A), K(7).value)
class E(enum.Enum, boundary=STRICT):
    a = 1
print(hasattr(E, "_boundary_"), E.a)
class P:
    def __init_subclass__(cls, /, flavor=None, **kw):
        cls.flavor = flavor
        super().__init_subclass__(**kw)
class Q(P, flavor="x"): pass
print(Q.flavor)
try:
    class R(P, flavor="y", other=1): pass
except TypeError as e:
    print("TE", e)
def f():
    n = 5
    class L(P, flavor=n * 2): pass
    return L.flavor
print(f())
class W(Flag, boundary=None):
    X = 1
print(W._boundary_)

import enum
from enum import STRICT, KEEP
print(repr(enum.Flag._boundary_), repr(STRICT), enum.Flag._boundary_ is STRICT, enum.STRICT is STRICT)
print(enum.FlagBoundary.STRICT is STRICT, enum.IntFlag._boundary_ is KEEP)

class P:
    def __init_subclass__(cls, **kw):
        super().__init_subclass__()
        print("ok", cls.__name__)
class Q(P): pass
class P2:
    def __init_subclass__(cls, **kw):
        try:
            super().__init_subclass__(1)
        except TypeError as e:
            print(e)
class Q2(P2): pass
class P3:
    @classmethod
    def __init_subclass__(cls, a, b=2):
        print(cls.__name__, a, b)
class Q3(P3, a=1): pass
class Q4(P3, b=5, a=0): pass
try:
    class Q5(P3): pass
except TypeError as e:
    print("TE", e)
class M(type):
    def __new__(m, n, b, ns, **kw):
        c = super().__new__(m, n, b, ns, **kw)
        c.kw = kw
        return c
class Base:
    def __init_subclass__(cls, tag=None):
        cls.tag = tag
class Z(Base, metaclass=M, tag="t"): pass
print(Z.tag, Z.kw)
class Z2(Z, tag="u"): pass
print(Z2.tag, Z2.kw, type(Z2).__name__)
import abc
class AB(abc.ABC):
    def __init_subclass__(cls, k=0): cls.k = k
class AC(AB, k=4): pass
print(AC.k)
try:
    class AD(metaclass=abc.ABCMeta, x=1):
        pass
except TypeError as e:
    print("TE", e)
