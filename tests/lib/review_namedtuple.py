# collections.namedtuple in the dynamic build: fields, defaults, helpers, subclasses, reprs, errors
from collections import namedtuple

P = namedtuple("P", "x y")
p = P(1, y=2)
print(p, p.x, p.y, p[0], p[-1], len(p), tuple(p), list(p))
a, b = p
print(a + b, p == (1, 2), p == P(1, 2), hash(p) == hash((1, 2)))
print(P._fields, P._field_defaults, p._asdict())
print(p._replace(y=5), P._make([7, 8]), P._make(iter([3, 4])))
print(P.__name__, P.__qualname__, P.__module__)

Q = namedtuple("Q", ["a", "b", "c"], defaults=[10, 20])
print(Q(1), Q(1, 2), Q(1, 2, 3), Q._field_defaults)

R = namedtuple("R", "def class _x", rename=True)
print(R._fields, R(1, 2, 3))


class V(P):
    __slots__ = ()

    def norm(self):
        return abs(self.x) + abs(self.y)

    def __repr__(self):
        return f"V<{self.x},{self.y}>"


v = V(-3, 4)
print(v, v.norm(), v.x, isinstance(v, P), isinstance(v, tuple), type(v._replace(x=1)).__name__)

for f in [
    lambda: P(1),
    lambda: P(1, 2, 3),
    lambda: P(1, 2, z=3),
    lambda: p._replace(z=1),
    lambda: setattr(p, "x", 5),
    lambda: namedtuple("Bad", "a a"),
    lambda: namedtuple("Bad", "1a"),
    lambda: P._make([1]),
    lambda: V(1),
    lambda: P(),
    lambda: P(1, x=2),
    lambda: namedtuple("T3", "a b c")(),
]:
    try:
        f()
    except Exception as e:
        print(type(e).__name__ + ":", e)

pts = [P(i, i * i) for i in range(5)]
print(sorted(pts, key=lambda q: -q.y)[0], max(pts), sum(q.x for q in pts))
print({p: 1}[P(1, 2)], p in {(1, 2)}, p < P(1, 3), p + (9,), p * 2)
