from enum import Flag, IntFlag, Enum, StrEnum, auto

class P(Flag):
    R = auto()
    W = auto()
    RW = R | W
print(P.RW, P.RW.value)

class C(Flag):
    A = auto()
    B = auto()
    D = auto()
    AB = A | B
    AD = A & D
print(C.A, C.B, C.D, C.AB, C.AD, [m.name for m in C])

class E(Enum):
    A = auto()
    B = auto()
    C = A
print(E.A, E.B, E.C, [m.name for m in E])

class I(IntFlag):
    A = auto()
    B = auto()
    C = A | B
print(I.A, I.B, I.C, I.C.value)

class S(StrEnum):
    A = auto()
    Bb = auto()
print(S.A, S.Bb)

class N(Enum):
    A = 5
    B = auto()
    C = auto()
print(N.A.value, N.B.value, N.C.value)

a = auto()
class F(Flag):
    X = a
    Y = a
print(F.X, F.Y, [m.name for m in F])

try:
    auto() | auto()
except TypeError as e:
    print(e)
