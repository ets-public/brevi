# Enum class __dict__ and dir() (CPython 3.13 order), Enum.__format__/__reduce_ex__, staticmethod and
# classmethod as types, Flag operators (IntFlag across classes, inverted values, None members), the
# functional API through the module, and an enum class __dict__ read inside __init_subclass__.
from enum import Enum, IntEnum, Flag, IntFlag, StrEnum, ReprEnum, auto
class A(Enum):
    X = 1
    Y = 2
class B(IntEnum):
    X = 1
class C(Flag):
    R = 1
    W = 2
class D(IntFlag):
    R = 1
    W = 2
class E(StrEnum):
    a = auto()
class F(str, Enum):
    def __new__(cls, v):
        o = str.__new__(cls, v)
        o._value_ = v
        return o
    p = 'p'
class G(float, Enum):
    q = 1.5
class H(int, ReprEnum):
    z = 3
class I(Enum):
    def __init_subclass__(cls, **kw):
        super().__init_subclass__(**kw)
class J(I):
    k = 1
K = Enum('K', 'a b')
for c in (A, B, C, D, E, F, G, H, J, K):
    print(c.__name__, list(c.__dict__))
    print(' dirm', dir(list(c)[0]))
    print(' dirc', dir(c))
class P(Enum):
    a = 1
print(Enum.__format__ is object.__format__, P.__format__ is Enum.__format__, Enum.__reduce_ex__ is object.__reduce_ex__)
print(format(P.a, '>6'), P.__format__(P.a, ''))
import enum
class Q(float, Enum):
    q = 1.5
class R(enum.IntEnum):
    r = 3
print(format(Q.q), format(Q.q, '>5'), format(R.r, 'x'), f"{R.r:03d}", P.a.__reduce_ex__(2), Enum.__dict__['__format__'] is Enum.__format__)
print(P.__format__(P.a, '^7') , '|')
class X:
    @staticmethod
    def s(): return 1
    @classmethod
    def c(cls): return cls.__name__
    t = staticmethod(len)
d = X.__dict__
print(staticmethod, classmethod, isinstance(d['s'], staticmethod), isinstance(d['c'], classmethod), type(d['c']))
print(type(d['s']) is staticmethod, type(d['c']) is classmethod, isinstance(d['s'], classmethod))
print(X.s(), X.c(), X().c(), X.t('ab'), callable(d['s']), type(staticmethod), isinstance(len, staticmethod))
print(d['s'].__func__ is X.s, type(X.s))
import enum
from enum import IntFlag, Flag
class Perm(IntFlag):
    R = 1 << 2
    W = 1 << 1
    X = 1 << 0
class NewPerm(IntFlag):
    R = 1 << 2
    W = 1 << 1
    X = 1 << 0
    def __str__(self):
        return self._name_
print(format(NewPerm.R, ''), format(NewPerm.R | Perm.X, ''), repr(NewPerm.R | Perm.X), str(NewPerm.R | NewPerm.X))
for i in list(Perm) + [Perm.R|Perm.W, Perm.R|Perm.X, Perm.W|Perm.X, Perm(7), Perm(0)]:
    print(repr(i), repr(~i), repr(~~i))
class FP(Flag):
    R = 4
    W = 2
    X = 1
for i in [FP.R|FP.X, FP.W, ~FP.W, FP(0)]:
    print(repr(i), repr(~i))
for et in (Flag, IntFlag):
    class ClosedXYZ(et):
        X = 4
        Y = 2
        Z = 1
        MASK = 7
    X, Y, Z = ClosedXYZ
    print(list(ClosedXYZ), ~X is Y|Z, ~(X|Y) is Z, ~ClosedXYZ.MASK is ClosedXYZ(0), ~ClosedXYZ(0) is (X|Y|Z), repr(~X), repr(Y|Z))
    class ClosedAB(et):
        A = 1
        B = 2
        MASK = 3
    A, B = ClosedAB
    print(~A is B, ~(A|B) is ClosedAB(0), ~ClosedAB.MASK is ClosedAB(0), ~ClosedAB(0) is (A|B), repr(~ClosedAB(0)), repr(A|B))
P2 = enum.IntFlag('P2', [])
print(len(P2), list(P2))
P3 = enum.IntFlag('P3', ())
print(len(P3))
import enum
F = enum.IntFlag
P2 = F('P2', [])
print(len(P2), list(P2))
print(len(enum.IntFlag('P3', ())), enum.Flag('P4', []), enum.IntEnum('P5', []))
from enum import Enum, Flag, IntEnum, IntFlag, StrEnum
for base in (Enum, Flag, IntEnum, IntFlag, StrEnum):
    class MyEnum(base):
        def __init_subclass__(cls, **kwds):
            super().__init_subclass__(**kwds)
            print(cls.__name__, list(cls.__dict__), cls.__dict__.get('_test', False))
            cls._test1 = 'MyEnum'
    class TheirEnum(MyEnum):
        def __init_subclass__(cls, **kwds):
            super(TheirEnum, cls).__init_subclass__(**kwds)
            cls._test2 = 'TheirEnum'
    class NoEnum(TheirEnum):
        ONE = auto() if False else ('one' if base is StrEnum else 1)
        def f(self): pass
    print(list(NoEnum.__dict__), TheirEnum.__dict__['_test1'], NoEnum.__dict__.get('_test1', False))
from enum import Flag
class FlagWithNoneMember(Flag):
    A = 1
    E = None
F = FlagWithNoneMember
print(F.A.value, F.E.value, list(F), F._flag_mask_, F._singles_mask_, F._all_bits_, repr(F.E), str(F.E), F(1), F(None), F._member_map_, F._value2member_map_)
for f in (lambda: F.A | F.E, lambda: F.E & F.A, lambda: F.A ^ F.E, lambda: ~F.E, lambda: F.E | F.E):
    try:
        print(f())
    except TypeError as e:
        print('TE', e)
print(F.A in F, bool(F.E), ~F.A, F(0))
