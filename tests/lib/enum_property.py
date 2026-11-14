import enum
from enum import Enum, IntEnum, StrEnum, Flag, IntFlag, ReprEnum, auto
class Base(Enum):
    @enum.property
    def first(self):
        return '%s is first!' % self.name
class Main(Base):
    first = auto()
    second = auto()
print(Main.first, Main.second.first, Main.first.first, [m.first for m in Main])
print(Main.second.name, Main.second.value, Main._member_type_, IntEnum._member_type_)
try: del Main.second.name
except AttributeError as e: print('AE', e)
try: Main.second.value = 3
except AttributeError as e: print('AE', e)
try: Main.name
except AttributeError as e: print('AE', e)
class F(float, ReprEnum):
    a = 1.5
print(str(F.a), format(F.a), repr(F.a), F._member_type_)
class N(IntEnum):
    numerator = 2
    other = 3
print(N.numerator, N.numerator.numerator, N.other.numerator)
class W(Enum):
    name = 1
    value = 2
print(W.name, W.name.name, W.value.value, W.name.value)
print(str(IntFlag), str(Flag), str(Enum), IntFlag.__mro__)
class P(IntFlag):
    R = 4
    W = 2
print(P.R | P.W, str(P.R), format(P.W), list(P))
