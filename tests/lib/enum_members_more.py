# Enum member creation (user __new__ and __init__, data types), _order_, dir(), reassigning and deleting
# members, reserved names, reversed(), copy, and str/format overrides, as CPython 3.13 does them
from enum import Enum, IntEnum
from datetime import date
import copy
class Color(Enum):
    RED = 1
    GREEN = 2
try:
    Color.RED = 5
except AttributeError as e:
    print("set", e)
try:
    del Color.RED
except AttributeError as e:
    print("del", e)
print(list(reversed(Color)))
print(copy.copy(Color.RED) is Color.RED, copy.deepcopy(Color.GREEN) is Color.GREEN)
try:
    class Bad(Enum):
        mro = 1
except ValueError as e:
    print("mro", e)
try:
    class Bad2(Enum):
        _foo_ = 1
except ValueError as e:
    print("sunder", e)
class P(Enum):
    __x = 1
    a = 2
print(list(P))
class D(Enum):
    first = 1978, 5, 26
    second = 2000, 1, 1
print(repr(D.first))
class AutoNumber(Enum):
    def __new__(cls):
        value = len(cls.__members__) + 1
        obj = object.__new__(cls)
        obj._value_ = value
        return obj
    first = ()
    second = ()
print(list(AutoNumber), AutoNumber(2))
class I(int, Enum):
    def __new__(cls, value, doc):
        obj = int.__new__(cls, value)
        obj._value_ = value
        obj.doc = doc
        return obj
    a = 1, "one"
    b = 2, "two"
print(I.a, I.b.doc, I(2), I.a + 1)
try:
    class S(Enum):
        def __new__(cls, value):
            return super().__new__(cls, value)
        a = 1
except TypeError as e:
    print("super", e)
def f():
    class S2(Enum):
        def __new__(cls, value):
            return super().__new__(cls, value)
        a = 1
try:
    f()
except TypeError as e:
    print("super2", e)
class Pl(Enum):
    def __init__(self, mass, radius):
        self.mass = mass
        self.radius = radius
    MERCURY = (3.303e+23, 2.4397e6)
    VENUS   = (4.869e+24, 6.0518e6)
print(Pl.VENUS.mass, Pl.MERCURY.value, Pl((3.303e+23, 2.4397e6)))
class SE(str, Enum):
    a = 1
    b = 'x'
print(repr(SE.a), SE('1'), SE.a == '1')
class IE(int, Enum):
    a = '10', 16
print(repr(IE.a), IE(16))
class D2(Enum):
    a = 1
    b = 1
print(D2.b is D2.a, list(D2))

from enum import Enum, IntEnum, Flag, auto
class C(Enum):
    a = 1
    def hi(self): pass
class N(Enum):
    def __new__(cls, v):
        o = object.__new__(cls); o._value_ = v; return o
    a = 1
class I(IntEnum):
    a = 1
class F(Flag):
    a = 1
print(dir(C)); print(dir(C.a)); print(dir(N)); print(dir(F)); print(dir(F.a)); print(dir(I) == sorted(set(dir(int)) | set(dir(C)) - {'a'} | {'a', '__new__'}), len(dir(I)))
class SuperEnum(Enum):
    def invisible(self):
        return "did you see me?"
class SubEnum(SuperEnum):
    sample = auto()
print('invisible' not in dir(SubEnum), 'invisible' in dir(SubEnum.sample))
class P(Enum):
    def __init__(self, v):
        self.desc = "x%s" % v
    one = 1
    @property
    def big(self): return 2
print(dir(P.one))

from enum import Enum, Flag
class A(Enum):
    _order_ = 'red green'
    red = 1
    green = 2
print(list(A), hasattr(A, '_order_'))
for body in ["_order_ = 'red green blue'\n    red = 1\n    green = 2", "_order_ = 'red green'\n    red = 1\n    green = 2\n    blue = 3", "_order_ = ['red', 'green']\n    red = 1\n    rouge = 1\n    green = 2"]:
    pass
try:
    class B(Enum):
        _order_ = 'red green'
        red = 1
        green = 2
        blue = 3
except TypeError as e:
    print(e)
class C(Enum):
    _order_ = 'red green'
    red = 1
    rouge = 1
    green = 2
print(list(C))
class F(Flag):
    _order_ = 'a b'
    a = 1
    b = 2
    ab = 3
print(list(F))

from enum import StrEnum
class X(str):
    def __str__(self): return "Y"
class SS(StrEnum):
    first = 'x'
    def __str__(self): return self.name.upper()
print("{}".format(X("a")), format(X("a")), f"{X('a')}", "{}".format(SS.first), "{0}".format(SS.first), "a{}".format(SS.first))

from enum import Enum, IntEnum, StrEnum, ReprEnum
class NS(IntEnum):
    first = 1
    def __str__(self): return self.name.upper()
class SS(StrEnum):
    first = 'x'
    def __str__(self): return self.name.upper()
class FF(float, ReprEnum):
    first = 1.5
    def __str__(self): return self.name.upper()
class ST(str, Enum):
    first = 'x'
    def __str__(self): return self.name.upper()
for m in (NS.first, SS.first, FF.first, ST.first):
    print(str(m), format(m), f"{m}", f"{m:>5}", "{}".format(m))
class G(IntEnum):
    item = 3
    def __repr__(self): return "%s.test" % (self._name_,)
    __str__ = object.__str__
class H(Enum):
    item = 3
    def __repr__(self): return "%s.test" % (self._name_,)
    __str__ = object.__str__
print(str(G.item), str(H.item), format(H.item), format(G.item))
