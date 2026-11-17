# unittest.mock objects stored on a class of the program as __set__, __delete__, __set_name__, __get__ or a plain
# name: the program's objects go into the mock sealed and come back as themselves (CPython 3.13.13 output)
from unittest.mock import Mock
class D:
    def __init__(self, n): self.n = n
class E:
    m = Mock(side_effect=lambda *a: len(a))
e = E()
print(e.m(1, 2), E.m(1))
E.k = Mock(side_effect=lambda *a: len(a))
print(e.k(1), E.k())
def f(*a): return [type(x).__name__ for x in a]
D.__set__ = Mock(side_effect=f)
D.__get__ = Mock(side_effect=f)
D.__delete__ = Mock(side_effect=f)
D.__set_name__ = Mock(side_effect=f)
class C:
    x = D(1)
c = C()
print(c.x, C.x)
c.x = 5
del c.x
print(D.__set_name__.call_count, D.__set__.call_count, D.__delete__.call_count, D.__get__.call_count)
print(D.__set__.side_effect is f)

from dataclasses import dataclass, field

class DS:
    pass
DS.__set__ = Mock()
DS.__get__ = Mock(return_value="got")
DS.__set_name__ = Mock()

@dataclass
class DC:
    i: DS = DS()
    j: int = field(default=DS(), init=False)

print(DS.__set_name__.call_count)
DS.__set__.reset_mock()
dc = DC(5)
print(DS.__set__.call_count, dc.i, DS.__get__.call_count)
dc.i = 10
print(DS.__set__.call_count)

held = []
class K:
    def __init__(self, n): self.n = n
class H:
    __set__ = Mock(side_effect=lambda o, v: held.append((o, v)))
class Box:
    h = H()
b = Box()
k = K(3)
b.h = k
print(held[0][0] is b, held[0][1] is k, held[0][1].n)
m = Mock(side_effect=lambda x: x)
print(m(k) is k)
log = []
class S:
    pass
S.__set__ = Mock(side_effect=lambda *a: log.append(f(*a)))
S.__delete__ = Mock(side_effect=lambda *a: log.append(f(*a)))
class T:
    s = S()
t = T()
t.s = 1
del t.s
print(log)
