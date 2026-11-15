# dataclasses reads a string annotation as CPython 3.13's dataclasses._is_type does: the optional module name and
# the name are looked up in the class's module, so 'typing.ClassVar' needs both `typing` and `ClassVar` bound there.
# The strings come from a variable, so only the run-time value can tell.
import dataclasses
import typing
from dataclasses import dataclass, field, fields, InitVar, KW_ONLY
from typing import ClassVar

for typestr in ('ClassVar[int]', ' ClassVar ', 'typing.ClassVar[str]', 'typing .ClassVar[str]', 'typing. ClassVar',
                'typing.ClassVar.[int]', 'typing.ClassVar+', 'InitVar', 'dataclasses.InitVar [ str]',
                'dataclasses.InitVar+', 'int', 'List[int]', 'foo.ClassVar', 'dataclasses.ClassVar', 'typing.InitVar',
                '', '+ClassVar'):
    @dataclass
    class C:
        x: typestr = 0
    kind = C.__dataclass_fields__['x']._field_type
    print(repr(typestr), [f.name for f in fields(C)], kind)


@dataclass
class K:
    a: int
    _: 'dataclasses.KW_ONLY'
    b: int = 2


print(K(1, b=3), [f.kw_only for f in fields(K)])


@dataclass
class I:
    a: int
    v: 'InitVar[int]'

    def __post_init__(self, v):
        print('post_init', v)


i = I(1, 5)
print(i, hasattr(i, 'v'))
