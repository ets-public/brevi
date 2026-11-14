# dataclasses: a bare ClassVar is no field, ClassVar[C] prints as CPython's, every dataclass has __replace__,
# star bases, make_dataclass with ClassVar fields and namespace methods, and copying a frozen class with
# __slots__ of its own runs its __setattr__ (FrozenInstanceError), as CPython 3.13
import copy
import typing
from dataclasses import FrozenInstanceError, dataclass, field, fields, make_dataclass
from typing import ClassVar


@dataclass
class C:
    x: int
    y: int = 10
    z: ClassVar[int] = 1000
    s: ClassVar = 4000
    t: typing.ClassVar = 5


c = C(5)
print(repr(c), len(fields(C)), len(C.__annotations__))
print(vars(C)['__annotations__'])
print(ClassVar[int] == ClassVar[int], ClassVar[int] == ClassVar[str], hash(ClassVar[int]) == hash(ClassVar[int]))
print([k for k in vars(C) if k.startswith('__r')], C.__replace__ is C.__dict__['__replace__'])
print(C.__replace__.__qualname__, C.__replace__.__module__, C.__replace__(c, y=3))

M = make_dataclass('M', [('x', int), ('y', ClassVar[int], 10), ('z', ClassVar[int], field(default=20))],
                   namespace={'add_one': lambda self: self.x + 1})
m = M(1)
print(vars(m), len(fields(m)), M.y, M.z, m.add_one())


@dataclass
class NotFrozen:
    pass


@dataclass(frozen=True)
class Frozen:
    pass


for bases in ((NotFrozen, Frozen), (Frozen, NotFrozen)):
    try:
        @dataclass
        class Child(*bases):
            pass
        print('made', [b.__name__ for b in Child.__mro__])
    except TypeError as e:
        print('TypeError', e)


@dataclass(frozen=True)
class U:
    __slots__ = ('s',)
    s: str


for f in (copy.copy, copy.deepcopy):
    try:
        f(U('a'))
    except FrozenInstanceError as e:
        print(f.__name__, 'FrozenInstanceError', e)


@dataclass(frozen=True, slots=True)
class S:
    s: str


print(copy.copy(S('x')), copy.deepcopy(S('y')))
