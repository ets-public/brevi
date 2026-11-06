from dataclasses import dataclass, field, asdict, astuple, replace, fields, is_dataclass, KW_ONLY, InitVar, FrozenInstanceError
from typing import ClassVar, NamedTuple
@dataclass
class P:
    x: int
    y: int = 0
    tags: list = field(default_factory=list)
    count: ClassVar[int] = 0
p = P(1, 2, ["t"])
print(p, asdict(p), astuple(p), replace(p, y=9), P.count, {1})
print([f.name for f in fields(P)], is_dataclass(p), is_dataclass(P), is_dataclass(3), P.__match_args__)
@dataclass(frozen=True, order=True)
class Q:
    a: int
    b: str = "z"
print(sorted([Q(2), Q(1, "a"), Q(1)]), hash(Q(1)) == hash(Q(1)), Q(1) == Q(1), Q(1) < Q(2))
q = Q(1)
try:
    q.a = 5
except FrozenInstanceError as e:
    print(type(e).__name__, e, isinstance(e, AttributeError))
try:
    del q.a
except AttributeError as e:
    print(type(e).__name__, e)
@dataclass(kw_only=True)
class K:
    a: int
    b: int = 2
@dataclass
class W:
    a: int
    _: KW_ONLY
    b: int = 3
    c: int
print(K(a=1), W(1, b=4, c=5), W(2, c=0))
for bad in [lambda: K(1), lambda: W(1), lambda: P(), lambda: P(1, 2, 3, 4), lambda: P(1, z=3), lambda: P(1, x=2)]:
    try:
        bad()
    except TypeError as e:
        print(e)
@dataclass
class I:
    a: int
    scale: InitVar[int] = 1
    total: int = field(init=False, default=0)
    def __post_init__(self, scale):
        self.a *= scale
        self.total = self.a + 1
print(I(3, 2), I(3), [f.name for f in fields(I)], replace(I(2, 5), a=4))
@dataclass(eq=False)
class R:
    a: int
@dataclass
class F:
    a: int = field(default=5, repr=False, compare=False)
    b: int = field(default=1, metadata={"u": 1})
print(R(1) == R(1), F(), F(1) == F(2), fields(F)[1].metadata["u"], fields(F)[0].default, F.a)
@dataclass
class B:
    x: int
    y: int = 7
@dataclass
class C(B):
    z: int = 1
    x: int = 4
print(C(), C(2, 3, 9) == C(2, 3, 9), [f.name for f in fields(C)], asdict(C()))
try:
    @dataclass
    class M:
        xs: list = []
except ValueError as e:
    print(e)
try:
    @dataclass
    class O:
        a: int = 1
        b: int
except TypeError as e:
    print(e)
class Pt(NamedTuple):
    x: int
    y: int
@dataclass
class Shape:
    pts: list[Pt]
    meta: dict = field(default_factory=dict)
s = Shape([Pt(1, 2)], {"k": [P(3)]})
print(asdict(s), astuple(s))
@dataclass(unsafe_hash=True)
class U:
    v: int
print(hash(U(3)) == hash((3,)), {U(1): 2}[U(1)])
@dataclass(init=False)
class NI:
    v: int = 3
print(NI(), NI().v)
@dataclass(repr=False)
class NR:
    v: int
print(type(repr(NR(1))).__name__)
