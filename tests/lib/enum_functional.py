from enum import Enum, IntEnum, Flag, StrEnum, auto
A = Enum('A', 'x y, z')
print(list(A), A.x, A(2), A.__qualname__, A.__module__)
B = IntEnum('B', [('p', 5), ('q', 7)], start=3)
print(list(B), B(7), B.q + 1)
C = Flag('C', 'r w x')
print(list(C), C.r | C.x)
D = StrEnum('D', 'Alpha Beta')
print(list(D))
E2 = Enum('E2', {'a': 10, 'b': 20}, qualname='X.E2')
print(E2.__qualname__, E2(20))
F = Enum('F', ('a', 'b'), start=0)
print(list(F))
class G(Enum):
    a = 1
    @classmethod
    def _missing_(cls, v):
        if v == 'one': return cls.a
print(G('one'))
try: G(5)
except ValueError as e: print(e)
try: Enum('Q')
except TypeError as e: print(e)
class H(Enum):
    def _generate_next_value_(name, start, count, last):
        return name + '!'
    a = auto()
    b = auto()
print(list(H))
def f():
    class I(Enum):
        a = 1
    try: I(3)
    except ValueError as e: print(e)
f()
