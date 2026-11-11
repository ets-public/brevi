import collections, dataclasses, enum, fractions
class A:
    def __add__(s, o): return 'A.add'
    def __radd__(s, o): return 'A.radd'
class B(A):
    def __radd__(s, o): return 'B.radd'
class C(B): pass
class D(A): pass
print(A() + B(), A() + C(), A() + D(), B() + A(), B() + C())
class N:
    def __add__(s, o): return NotImplemented
    def __radd__(s, o): print('N.radd called'); return NotImplemented
try: N() + N()
except TypeError as e: print('TE', e)
class M(N):
    def __radd__(s, o): print('M.radd'); return NotImplemented
try: N() + M()
except TypeError as e: print('TE', e)
class P:
    def __sub__(s, o): return NotImplemented
class Q(P):
    def __rsub__(s, o): return 'Q.rsub'
print(P() - Q())
class E:
    def __eq__(s, o): return 'E.eq'
class F(E):
    def __eq__(s, o): return 'F.eq'
print(E() == F(), F() == E())
class G:
    def __eq__(s, o): print('G.eq', type(s).__name__); return NotImplemented
    def __lt__(s, o): print('G.lt', type(s).__name__); return NotImplemented
    def __gt__(s, o): print('G.gt', type(s).__name__); return NotImplemented
class H(G): pass
print(G() == H(), H() == G())
try: G() < H()
except TypeError as e: print('TE', e)
class K:
    def __ne__(s, o): return NotImplemented
    def __eq__(s, o): return True
print(K() != K(), K() != 1)
class Mul:
    def __mul__(s, o): return 'mul'
class Mul2(Mul):
    __rmul__ = Mul.__mul__
print(Mul() * Mul2())
class Iv(list):
    pass
print(Iv([1]) + [2], [1] + Iv([2]))
class Iv2(list): pass
print(Iv2([1]) == [1], [1] == Iv2([1]), Iv2([1]) != [2], Iv2([1]) < [2])
@dataclasses.dataclass
class P: x: int
print(P(1) == P(1), P(1) != P(2), P(1) != P(1))
class E(enum.Enum):
    A = 1
print(E.A == E.A, E.A != E.A, E.A != 1)
NT = collections.namedtuple('NT', 'a b')
print(NT(1, 2) == (1, 2), NT(1, 2) != (1, 3))
class O: pass
o = O(); print(o == o, o != o, O() != O(), o == 1, o != 1)
class Q:
    def __eq__(s, o): return isinstance(o, Q)
print(Q() != Q(), Q() != 1, 1 != Q())
class Cnt(collections.Counter): pass
print(Cnt('ab') == collections.Counter('ab'), Cnt('a') != Cnt('b'))
try: O() < O()
except TypeError as e: print('TE', e)
class R:
    def __lt__(s, o): return NotImplemented
try: R() < R()
except TypeError as e: print('TE', e)
try: R() < 1
except TypeError as e: print('TE', e)
class V(int):
    def __radd__(s, o): return 'V.radd'
print(1 + V(2), V(2) + 1)
x = [Q(), Q()]; print(Q() in x, x.count(Q()), x.index(Q()))
print(sorted([fractions.Fraction(1, 2), 0.25]))
if O() != O(): print('ne ok')
if not (o != o): print('ne self ok')
