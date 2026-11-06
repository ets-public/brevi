from typing import NamedTuple
class Pt(NamedTuple):
    """A point."""
    x: int
    y: int = 0
    def norm(self) -> int:
        return abs(self.x) + abs(self.y)
class Q(NamedTuple):
    a: str
    b: float = 1.5
p = Pt(1)
print(p, p._replace(y=3), p._asdict(), Pt._fields, p.x, p[1], p.norm(), p == (1, 0), Pt.__doc__, Q.__doc__, Q._field_defaults, Q._make(["z", 2]))
a, b = Pt(5, 6)
print(a, b, len(p), sorted([Pt(2), Pt(1, 9)]))
try:
    class Bad(NamedTuple):
        x: int = 1
        y: int
except TypeError as e:
    print(e)
P2 = NamedTuple("P2", [("u", int), ("v", str)])
print(P2(1, "a"))
