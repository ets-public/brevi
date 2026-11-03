# Dataclasses print as CPython's generated __repr__, also inside lists, dicts and tuples, and after __str__.
from dataclasses import dataclass, field
from typing import List


@dataclass
class P:
    x: int
    y: int = 0


@dataclass
class Item:
    name: str
    price: float
    tags: list[str] = field(default_factory=list)


@dataclass
class Box:
    p: P
    items: list[Item]


@dataclass(order=True)
class Base:
    a: int
    b: str = "q"


@dataclass(frozen=True)
class Pt:
    x: int
    y: int


@dataclass
class Acc:
    owner: str
    secret: str = field(repr=False, default="s")
    hist: List[int] = field(default_factory=list)


@dataclass
class Rp:
    v: int

    def __repr__(self):
        return f"Rp<{self.v}>"


@dataclass
class S:
    v: int

    def __str__(self):
        return f"S:{self.v}"


class Both:
    def __init__(self, v):
        self.v = v

    def __str__(self):
        return f"str {self.v}"

    def __repr__(self):
        return f"Both({self.v})"


p = P(1)
print(p)
print([P(1, 2), P(3)])
print({"a": P(5)})
print((P(1), 2))
it = Item("pen", 1.5, ["x", "y"])
print(it)
print(Box(P(2, 3), [it]))
print(str(p), repr(p))
print(f"{p} {p!r}")
print(Rp(3), [Rp(4)])
print(S(5), [S(6)], repr(S(7)), f"{S(8)!r}")
print(Both(1), [Both(2)], repr(Both(3)))
print(P(1) == P(1), P(1) == P(2))
q = P(4, 5)
q.x = 9
print(q)
print(sorted([Base(3), Base(1)]))
print(Pt(1, 2))
a = Acc("bob")
a.hist.append(3)
print(a)
print([None, Pt(0, 0)])
print(str([Pt(1, 1)]), repr("x"))
