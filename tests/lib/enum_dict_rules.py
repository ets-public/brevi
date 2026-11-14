# EnumDict's rules for an Enum class body (enum.py 3.13), and the data type's methods an Enum replaces.
import warnings
from collections import namedtuple
from dataclasses import dataclass, field
from enum import Enum, IntEnum, auto, property as eprop
from functools import partial


def attempt(make):
    try:
        make()
        print("made")
    except (TypeError, ValueError) as e:
        print(type(e).__name__, str(e).split(" at 0x")[0])


def twice():
    class C(Enum):
        red = 1
        green = 2
        red = 4


def method_over_member():
    class C(Enum):
        red = 1

        def red(self):
            return "red"


def member_over_property():
    class C(Enum):
        @eprop
        def red(self):
            return 1
        red = 2


def ignore_after():
    class C(Enum):
        A = 1
        _ignore_ = ["A"]


def auto_twice():
    class C(Enum):
        A = auto()
        A = auto()


for f in (twice, method_over_member, member_over_property, ignore_after, auto_twice):
    attempt(f)


class Period(Enum):
    _ignore_ = "i j"
    ZERO = 0
    for i in range(3):
        pass
    j = 9


print(list(Period), hasattr(Period, "i"), hasattr(Period, "j"))

# a value of a builtin type is never a descriptor (frozenset has no __delete__)
print([hasattr(x, n) for x in (frozenset(), set(), {}, [], (), 1, 1.5, True, "s", range(1)) for n in ("__get__", "__delete__", "__len__")])


class Dirs(Enum):
    DOWN = frozenset({"sc"})
    UP = frozenset({"cs"})


print(list(Dirs), Dirs({"sc"}) is Dirs.DOWN)


def pair(a, b=5):
    return a, b


with warnings.catch_warnings(record=True) as w:
    warnings.simplefilter("always")

    class P(Enum):
        a = 1
        b = partial(pair)
print([x.category.__name__ for x in w], list(P), P.b(2))

T = namedtuple("T", "x y")


class NT(Enum):
    A = T(1, 2)


class NTMix(T, Enum):
    A = 1, 2


class NTNew(Enum):
    def __new__(cls, x, y):
        m = object.__new__(cls)
        m.x, m.y = x, y
        return m
    A = T(3, 4)


print(repr(NT.A), repr(NTMix.A), NTMix.A.x, str(NTMix.A), NTNew.A.x, NTNew.A.y)


@dataclass
class Size:
    size: str
    legs: int
    tail: bool = field(repr=False, default=True)


class Creature(Size, Enum):
    DOG = ("medium", 4)


@dataclass(repr=False)
class Plain:
    a: int

    def __repr__(self):
        return "plain!"


class Entries(Plain, Enum):
    ONE = 1


print(repr(Creature.DOG), repr(Entries.ONE), Entries.ONE.value == Plain(1))


class MyInt(int):
    def __new__(cls, value):
        return super().__new__(cls, value)


class HexMixin:
    def __repr__(self):
        return hex(self)


class HexEnum(HexMixin, MyInt, Enum):
    __repr__ = HexMixin.__repr__


class Foo(HexEnum):
    TEST = 10


print(repr(Foo.TEST), isinstance(Foo.TEST, MyInt), Foo._member_type_ is MyInt)


def scope():
    class Inner:
        __qualname__ = "Renamed"
    print(Inner.__qualname__, repr(Inner))


scope()
