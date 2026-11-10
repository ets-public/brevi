from enum import Enum, IntEnum, Flag, IntFlag, StrEnum, auto
class P(Flag):
    R = auto(); W = auto(); X = auto()
    RW = R | W
    ALL = R | W | X
print(list(P), P.ALL, P.RW.value, P(5))
class Q(IntFlag):
    A = auto(); B = auto(); AB = A | B; C = auto()
print(list(Q), Q.C.value, Q.AB)
class C(Enum):
    a = auto(); b = 10; c = auto(); d = auto()
    def m(self): return self.value * 2
    _hidden = 5
print([(x.name, x.value) for x in C], C.c.m())
class S(StrEnum):
    Foo = auto(); BAR = auto()
print(list(S), S.BAR.value)
class N(IntEnum):
    x = auto(); y = x + 5 if False else 7; z = auto()
print(list(N))
class Plain:
    a = 1; b = a + 1
print(Plain.b)
class Base(Enum):
    pass
class D(Base):
    p = auto(); q = auto()
print(list(D))
a1 = auto()
class E(Enum):
    one = a1; two = auto()
print(list(E))
class F(Flag):
    A = 1; B = 2; C = auto(); D = A | C
print(list(F), F.D)
class G(Enum):
    _a = 1; d = 4
print(list(G))
