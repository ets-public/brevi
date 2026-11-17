# Pickling the program's own int and tuple subclasses and namedtuples: CPython 3.13.13's bytes at every protocol,
# and an Enum member with an int-subclass data type loads as the member.
import pickle, builtins as bltns
from enum import Enum
from collections import namedtuple

class NamedInt(int):
    __qualname__ = 'NamedInt'
    def __new__(cls, *args):
        _args = args
        name, *args = args
        if len(args) == 0:
            raise TypeError("name and value must be specified")
        self = int.__new__(cls, *args)
        self._intname = name
        self._args = _args
        return self
    def __getnewargs__(self):
        return self._args
    @bltns.property
    def __name__(self):
        return self._intname
    def __repr__(self):
        return "{}({!r}, {})".format(type(self).__name__, self.__name__, int.__repr__(self))

class NEI(NamedInt, Enum):
    __qualname__ = 'NEI'
    x = ('the-x', 1)
    y = ('the-y', 2)

class Plain(int):
    pass

class PT(tuple):
    pass

P = namedtuple('P', 'a b')

for proto in range(pickle.HIGHEST_PROTOCOL + 1):
    ni = NamedInt('test', 5)
    d = pickle.dumps(ni, proto)
    print(proto, d)
    l = pickle.loads(d)
    print(type(l).__name__, repr(l), l == 5, l.__dict__)
    print(pickle.loads(pickle.dumps(NEI.y, proto)) is NEI.y)
    p = Plain(7); p.z = 1
    d = pickle.dumps(p, proto); print(d)
    l = pickle.loads(d); print(type(l).__name__, l, l.__dict__)
    d = pickle.dumps(PT((1, 2)), proto); print(d)
    l = pickle.loads(d); print(type(l).__name__, l)
    d = pickle.dumps(P(1, [2]), proto); print(d)
    l = pickle.loads(d); print(type(l).__name__, l, type(l) is P)
