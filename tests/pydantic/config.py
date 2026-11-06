# Brevi's own pydantic: model_config, aliases, frozen and private attributes, enums, inheritance.
from enum import Enum, IntEnum
from typing import ClassVar, Optional
from pydantic import BaseModel, ConfigDict, Field, PrivateAttr, ValidationError, PositiveInt, StrictStr


class Color(str, Enum):
    RED = "red"
    GREEN = "green"


class Level(IntEnum):
    LOW = 1
    HIGH = 2


class Strict(BaseModel):
    model_config = ConfigDict(extra="forbid", str_strip_whitespace=True, str_to_lower=True)
    name: str
    color: Color = Color.RED
    level: Level = Level.LOW


print(Strict(name="  HeLLo ", color="green", level="2"))
for d in [{"name": "x", "other": 1}, {"name": "x", "color": "blue", "level": 3}]:
    try:
        Strict(**d)
    except ValidationError as e:
        print(e)


class Open(BaseModel):
    model_config = ConfigDict(extra="allow", use_enum_values=True)
    color: Color = Color.RED


o = Open(color="green", size=3, label="x")
print(repr(o), o.model_extra, o.model_dump(), o.size, o.model_dump_json())


class Frozen(BaseModel):
    model_config = ConfigDict(frozen=True)
    a: int
    b: tuple[int, ...] = ()


f = Frozen(a=1, b=[1, 2])
print(f, hash(f) == hash(Frozen(a=1, b=(1, 2))), {f: 1}[Frozen(a=1, b=(1, 2))])
try:
    f.a = 2
except ValidationError as e:
    print(e)


class Live(BaseModel):
    model_config = ConfigDict(validate_assignment=True, populate_by_name=True)
    count: PositiveInt = Field(1, alias="n")
    label: Optional[StrictStr] = None
    kind: ClassVar[str] = "live"
    _hits: int = PrivateAttr(default=0)

    def hit(self):
        self._hits += 1
        return self._hits


x = Live(n=3)
y = Live(count=4)
print(x, y, Live.kind, x.hit(), x.hit(), y.hit())
x.count = "7"
print(x.count, sorted(x.model_fields_set))
for bad in [("count", 0), ("label", 5), ("nope", 1)]:
    try:
        setattr(x, *bad)
    except (ValidationError, ValueError) as e:
        print(type(e).__name__, e)
try:
    x.kind = "dead"
except AttributeError as e:
    print(e)


class Base(BaseModel):
    id: int
    name: str = "base"


class Child(Base):
    name: str = "child"
    extra: list[Base] = []


c = Child(id=1, extra=[{"id": 2}, Base(id=3), Child(id=4)])
print(repr(c), list(Child.model_fields), isinstance(c, Base))
print(c.model_dump())


class FromObj(BaseModel):
    model_config = ConfigDict(from_attributes=True)
    id: int
    name: str


class Row:
    def __init__(self):
        self.id = "9"
        self.name = "row"


print(FromObj.model_validate(Row()))


class StrictModel(BaseModel):
    model_config = ConfigDict(strict=True)
    n: int
    s: str


for d in [{"n": 1, "s": "a"}, {"n": "1", "s": 1}, {"n": True, "s": b"x"}]:
    try:
        print(StrictModel(**d))
    except ValidationError as e:
        print(e)


class Thing:
    def __init__(self, n):
        self.n = n

    def __repr__(self):
        return "Thing(%d)" % self.n


class Holder(BaseModel):
    model_config = ConfigDict(arbitrary_types_allowed=True, title="Box")
    thing: Thing
    more: list[Thing] = []
    _area: int = PrivateAttr(0)

    def model_post_init(self, context):
        self._area = self.thing.n * len(self.more)
        if self._area > 10:
            raise ValueError("too big")


h = Holder(thing=Thing(3), more=[Thing(1), Thing(2)])
print(h, h._area, h.model_dump())
for d in [{"thing": 5}, {"thing": Thing(6), "more": [Thing(1), Thing(1)]}]:
    try:
        Holder(**d)
    except ValidationError as e:
        print(e)
try:
    h.model_dump_json()
except Exception as e:
    print(type(e).__name__, e)
