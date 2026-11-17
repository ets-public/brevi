# __reduce__, __reduce_ex__, __getnewargs__ and __getnewargs_ex__ of the program's classes in pickle and copy; the
# program's __new__ and __init__ run inside pickle.loads, in CPython's order; identity kept across the load.
import pickle, copy
from collections import namedtuple

class NI(int):
    def __new__(cls, *args):
        name, *rest = args
        if not rest:
            raise TypeError("name and value must be specified")
        self = int.__new__(cls, *rest)
        self._n = name
        self._args = args
        print("new", args)
        return self
    def __getnewargs__(self):
        return self._args

class RI(int):
    def __new__(cls, name, v):
        self = int.__new__(cls, v)
        self.name = name
        return self
    def __reduce__(self):
        return self.__class__, (self.name, int(self))

class RX(tuple):
    def __reduce_ex__(self, proto):
        print("reduce_ex", proto)
        return (RX, (tuple(self),))

class KW(int):
    def __new__(cls, v, *, tag):
        self = int.__new__(cls, v)
        self.tag = tag
        return self
    def __getnewargs_ex__(self):
        return (int(self),), {'tag': self.tag}

class Obj:
    def __init__(self, a):
        print("init", a)
        self.a = a
    def __reduce__(self):
        return (Obj, (self.a,))

class GA:
    def __new__(cls, x):
        o = object.__new__(cls)
        o.x = x
        return o
    def __getnewargs__(self):
        return (self.x,)

P = namedtuple('P', 'a b')
class Q(P):
    def total(self):
        return self.a + self.b[0]

shared = [1, 2]
s = "share" + str(len(shared))
for proto in range(6):
    print("proto", proto)
    n = NI(s, 3)
    out = pickle.loads(pickle.dumps([n, n, n._args, s], proto))
    print(out, out[0] is out[1], out[0]._args is out[2], out[0]._n is out[3], type(out[0]).__name__)
    r = pickle.loads(pickle.dumps(RI('r', 4), proto)); print(type(r).__name__, r, r.name)
    x = pickle.loads(pickle.dumps(RX((1, 2)), proto)); print(type(x).__name__, x)
    if proto >= 4:
        k = pickle.loads(pickle.dumps(KW(5, tag='t'), proto)); print(type(k).__name__, k, k.tag)
    elif proto < 2:
        print(pickle.dumps(KW(5, tag='t'), proto)[:20])
    o = pickle.loads(pickle.dumps(Obj([1]), proto)); print(type(o).__name__, o.a)
    g = pickle.loads(pickle.dumps(GA(9), proto)); print(type(g).__name__, g.x)
    t = pickle.loads(pickle.dumps([P(1, shared), shared, Q(2, shared)], proto))
    print(t, t[0].b is t[1], t[2].b is t[1], type(t[0]) is P, type(t[2]) is Q, t[2].total())
try:
    pickle.loads(pickle.dumps(NI('a', 1), 2).replace(b'\x05', b'\x05'))
except Exception as e:
    print(type(e).__name__, e)
c = copy.copy(NI('c', 6)); print(type(c).__name__, c, c._n)
d = copy.deepcopy([NI('d', 7)]); print(d, d[0]._args)
try:
    copy.copy(RI('z', 1))
except Exception as e:
    print(type(e).__name__, e)
print(copy.copy(RI('z', 1)).name)
class Bad(int):
    def __new__(cls, *a):
        if a and a[0] == 'fail':
            raise ValueError("no " + str(a))
        return int.__new__(cls, *a[1:])
    def __getnewargs__(self):
        return ('fail', int(self))
try:
    pickle.loads(pickle.dumps(Bad('ok', 3), 2))
except ValueError as e:
    print("ValueError", e)

class NI3(int):
    pass
class Plain3:
    pass
p = Plain3(); p.v = 1
n = NI3(4); n.p = p
r = pickle.loads(pickle.dumps([p, n], 2))
print(r[1].p is r[0], r[0].v)
T = namedtuple('T', 'x')
t = T(T(1))
print(pickle.loads(pickle.dumps(t)) == t)
