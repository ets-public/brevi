# hex/oct/bin/%x/range/round of int subclasses and __index__ objects; auto() in namedtuple Enum values;
# namedtuple-mixin Enum members through pickle; builtin functions through pickle; a class attribute named
# __name__; @builtins.property
import builtins as bltns
import pickle
from collections import namedtuple
from enum import Enum, auto


class HexInt(int):
    def __repr__(self):
        return hex(self)


class Sub(HexInt):
    pass


class Idx:
    def __index__(self):
        return 300


class BadIdx:
    def __index__(self):
        return 1.5


h = HexInt(26)
print(hex(h), oct(h), bin(h), repr(h), "%x %o %#X" % (h, h, h), round(h), type(round(h)).__name__)
print(hex(Sub(5)), list(range(Sub(3))), list(range(1, Idx(), Sub(100))), round(Sub(1234), -2))
print(hex(Idx()), "%x" % Idx())
for f in (lambda: hex(BadIdx()), lambda: "%x" % BadIdx(), lambda: hex(1.5), lambda: range(1.5)):
    try:
        f()
    except TypeError as e:
        print(e)


class F(float):
    pass


print(round(F(2.567), 2), round(F(2.5)))

T = namedtuple('T', 'index desc')


class Plain(Enum):
    first = T(auto(), 'money')
    second = T(auto(), 'show')
    third = T(5, auto())


print(Plain.first.value, Plain.second.value.index, Plain.third.value)


class Mixed(T, Enum):
    first = auto(), 'money'
    second = 2, 'show'


for p in range(pickle.HIGHEST_PROTOCOL + 1):
    x = pickle.loads(pickle.dumps(Mixed.first, protocol=p))
    print(p, x is Mixed.first, pickle.loads(pickle.dumps(Plain.second, protocol=p)) is Plain.second)

print(getattr is getattr, len is len)
for p in range(pickle.HIGHEST_PROTOCOL + 1):
    d = pickle.dumps([getattr, len, max], protocol=p)
    x = pickle.loads(d)
    print(d, x[0] is getattr, x[1] is len, x[2](3, 9))


class Named:
    @property
    def __name__(self):
        return 'inst'

    @bltns.property
    def other(self):
        return 'other'


print(Named.__name__, Named().__name__, type(Named()).__name__, Named().other)
Named.__name__ = 'Renamed'
Named.__qualname__ = 'RQ'
print(Named.__name__, Named.__qualname__, Named)
try:
    Named.__name__ = 3
except TypeError as e:
    print(e)
