# Enum.__new__ is what an enum with a data type of the program (a class with __new__) finds, and
# globals()['Name'] = value inside a function stores a global that pickle finds by name.
import pickle
from enum import Enum
class NamedInt(int):
    def __new__(cls, *args):
        name, *args = args
        self = int.__new__(cls, *args)
        self._intname = name
        return self
class NEI(NamedInt, Enum):
    x = ('the-x', 1)
    y = ('the-y', 2)
class Base(NamedInt, Enum):
    pass
class Sub(Base):
    z = ('zz', 5)
print(NEI.__new__ is Enum.__new__, Base.__new__ is Enum.__new__, Sub.__new__ is Enum.__new__)
print(NEI(1), NEI(2) is NEI.y, Sub(5), Sub.z._intname, NEI.x._intname, type(NEI.x._value_).__name__)
print(NEI.__dict__['__new__'] is Enum.__new__, Base.__dict__['__new__'] is Enum.__new__)
print(sorted(k for k in NEI.__dict__ if k.startswith('__n')))
try:
    NEI(3)
except ValueError as e:
    print(e)

def make():
    class Pt(tuple):
        __qualname__ = 'Pt'
    class Num(int):
        __qualname__ = 'Num'
        def __new__(cls, name, v):
            self = int.__new__(cls, v)
            self.label = name
            return self
        def __getnewargs__(self):
            return (self.label, int(self))
    class Col(Num, Enum):
        __qualname__ = 'Col'
        red = ('r', 1)
        blue = ('b', 2)
    globals()['Pt'] = Pt
    globals()['Num'] = Num
    globals()['Col'] = Col
    return Pt, Num, Col
made = make()
for proto in range(6):
    p = pickle.loads(pickle.dumps(made[0]((1, 2)), proto))
    n = pickle.loads(pickle.dumps(made[1]('n', 3), proto))
    print(type(p) is made[0], p, type(n) is made[1], n, n.label, pickle.loads(pickle.dumps(made[2].blue, proto)) is made[2].blue)
print(made[2].__new__ is Enum.__new__, made[0].__qualname__)
