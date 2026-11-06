from enum import Flag, IntFlag, auto, Enum, IntEnum, StrEnum
class P(Flag):
    R = auto()
    W = auto()
    X = auto()
rw = P.R | P.W
print(rw, P.R in rw, P.X in rw, repr(rw), bool(P(0)), list(rw), rw & P.W, ~P.R, repr(~P.R), len(rw), P(0), repr(P(0)), P(3) is rw, rw.name, rw.value, list(P), P.R | P.W | P.X, ~(P.R|P.W|P.X), repr(~(P.R|P.W|P.X)), P.R ^ P.W, rw == P(3), hash(rw)==hash(P(3)), P.W|P.R)
class Q(Flag):
    A = 1
    B = 4
    AB = 5
    C = 16
print(list(Q), Q.A|Q.B, repr(Q.A|Q.C), Q(21), list(Q(21)), ~Q.A, list(Q.__members__.keys()), repr(Q.AB), str(Q(0)), Q.A in Q.AB, len(Q))
try: P(8)
except ValueError as e: print(repr(str(e)))
class I(IntFlag):
    A = 1
    B = 2
print(I.A | I.B, int(I.A | I.B), I(3), repr(I(3)), I.A | 4, repr(I.A | 4), I.A + 1, ~I.A, repr(~I.A), I(0), repr(I(0)), f'{I.A|I.B}', I.A == 1, I(9), repr(I(9)), list(I(3)), I.A & 3, type(I.A & 3), 3 | I.A, repr(3|I.A), I.B > I.A, I.A ^ 3)
class F(Flag):
    NONE = 0
    B = 2
    A = 1
    AB = 3
print(list(F), len(F), list(F.__members__.keys()), F(0), repr(F(0)), F.NONE, F(3), list(F(3)), F.A|F.B, F._flag_mask_, F._singles_mask_, F._all_bits_, F.A in F, F(1) is F.A, ~F.A, ~F.NONE, repr(~F.NONE))
print(repr(F.A.name), F(0).name, F.A.value)
try: F.A | 1
except TypeError as e: print(e)
try: 1 in F.A
except TypeError as e: print(e)
class G(Flag):
    X = auto()
    Y = auto()
    Z = 8
    W = auto()
print(list(G), [g.value for g in G])
try:
    class H(Flag):
        a = 'x'
        b = auto()
except TypeError as e: print(e)
class J(IntFlag):
    R = 4
    W = 2
    X = 1
print(J.R|J.X, repr(J(7)), list(J(7)), J(7) == 7, J.R < 5, sorted([J.R, J.W]), str(J.R), '%d' % J.R, '%s' % J.R, J.R * 2, hash(J.R)==hash(4), -J.R)
class C(Enum):
    A = 1
print(f'{C.A:>6}|', '%s' % C.A, P, I, C)
class K(IntEnum):
    A = 1
    B = auto()
    C = 5
    D = auto()
print(K.A, repr(K.A), f'{K.B}', '%s' % K.A, '%d' % K.B, str(K.D), K.D.value, K.A + 1, K.A == 1, hash(K.A)==hash(1), K(5), K['B'], [c for c in K], K.A < K.B, int(K.B), type(K.A + 1), {K.A: 'x'}[1], format(K.A, '>4'), K.A.name, max(K), list(K.__members__), -K.C, ~K.A)
class S(StrEnum):
    X = auto()
    Yy = auto()
    Z = 'zz'
print(S.X, repr(S.X), f'{S.Yy}', S.Z.upper(), S.X == 'x', S('zz'), list(S), S.X + '!', '-'.join(S), sorted(S), format(S.Z, '>5'), len(S.X), {S.X: 1}['x'])
try:
    class Bad(StrEnum):
        A = 1
except TypeError as e: print(e)
try: K(9)
except ValueError as e: print(e)
