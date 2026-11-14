# enum's names beyond Enum and its kinds: EnumType, member/nonmember, verify, FlagBoundary, the helpers
import enum
from enum import Enum, EnumMeta, EnumType, Flag, IntFlag, IntEnum, auto, member, nonmember, verify, UNIQUE, CONTINUOUS, NAMED_FLAGS, EJECT, KEEP, STRICT, CONFORM, _iter_bits_lsb
class Color(Enum):
    R = 1
    G = 2
print(type(Color), type(Enum), isinstance(Color, EnumMeta), EnumMeta is EnumType, repr(EnumType), type(Color) is EnumType)
class S(metaclass=EnumMeta):
    pass
class E(S, Enum):
    A = 1
print(repr(E.A), E.__mro__, type(S), repr(S), len(S), list(S), bool(S))
print(repr(EJECT), repr(UNIQUE), str(UNIQUE), EJECT is enum.EJECT, list(enum.EnumCheck))
class Space(Flag):
    A = 1
print(Space._boundary_ is STRICT, Space._boundary_ in (EJECT, KEEP))
class MyTypes(Enum):
    i = int
    f = nonmember(float)
    s = str
print(list(MyTypes), MyTypes.f is float)
class Ex(Flag):
    A = auto()
    B = auto()
    ALL = nonmember(A | B)
print(list(Ex), Ex.ALL, type(Ex.ALL))
class Outer(Enum):
    a = 1
    @member
    class Inner(Enum):
        foo = 10
    @nonmember
    class In2(Enum):
        x = 1
print(list(Outer), isinstance(Outer.Inner, Outer), list(Outer.Inner.value), Outer.In2.x)
@verify(UNIQUE)
class Silly(Enum):
    one = 1
    name = 3
try:
    @verify(UNIQUE)
    class Dirtier(IntEnum):
        single = 1
        double = 1
        triple = 3
        turkey = 3
except ValueError as e: print(e)
try:
    @verify(CONTINUOUS)
    class Missing(Enum):
        FIRST = 3
        SECOND = 4
        THIRD = 11
        FORTH = 13
except ValueError as e: print(e)
try:
    @verify(CONTINUOUS)
    class Incomplete(Flag):
        FIRST = 4
        SECOND = 8
        THIRD = 16
        FORTH = 64
except ValueError as e: print(e)
try:
    @verify(NAMED_FLAGS)
    class Bizarre(Flag):
        b = 3
        c = 4
        d = 6
except ValueError as e: print(e)
try:
    @verify(NAMED_FLAGS)
    class Bizarre(IntFlag):
        c = 4
        d = 6
except ValueError as e: print(e)
print(enum.show_flag_values(3), list(_iter_bits_lsb(7)))
g = _iter_bits_lsb(-8)
try: list(g)
except ValueError as e: print(e)
print(enum._is_sunder('_a_'), enum._is_dunder('__a__'), enum._is_private('MyEnum', '_MyEnum__x'), enum._is_private('MyEnum', '_MyEnum__'))
class foo: pass
o = foo(); print(enum._is_descriptor(o)); o.__get__ = 1; print(enum._is_descriptor(o))
print(member.__doc__, type(verify(UNIQUE)))
try: nonmember()
except TypeError as e: print(e)
class Color(Enum):
    R = 1
print(type(Color).__name__, isinstance(Color, type), issubclass(EnumMeta, type), type(Flag), type(Color.R))
try: Color + 1
except TypeError as e: print(e)
try: len(5)
except TypeError as e: print(e)
print(type(enum.EJECT), enum.KEEP == 'keep', enum.UNIQUE.value, str(enum.EJECT), f"{enum.EJECT}")
print(repr(enum.FlagBoundary), [m.name for m in enum.FlagBoundary])
