# functools.total_ordering: each root derives the missing comparisons as CPython's helpers do
from functools import total_ordering


@total_ordering
class K0:
    def __init__(self, v):
        self.v = v

    def __eq__(self, o):
        return isinstance(o, K0) and self.v == o.v

    def __lt__(self, o):
        if not isinstance(o, K0):
            return NotImplemented
        return self.v < o.v


@total_ordering
class K1:
    def __init__(self, v):
        self.v = v

    def __eq__(self, o):
        return isinstance(o, K1) and self.v == o.v

    def __le__(self, o):
        if not isinstance(o, K1):
            return NotImplemented
        return self.v <= o.v


@total_ordering
class K2:
    def __init__(self, v):
        self.v = v

    def __eq__(self, o):
        return isinstance(o, K2) and self.v == o.v

    def __gt__(self, o):
        if not isinstance(o, K2):
            return NotImplemented
        return self.v > o.v


@total_ordering
class K3:
    def __init__(self, v):
        self.v = v

    def __eq__(self, o):
        return isinstance(o, K3) and self.v == o.v

    def __ge__(self, o):
        if not isinstance(o, K3):
            return NotImplemented
        return self.v >= o.v


for root, K in [("__lt__", K0), ("__le__", K1), ("__gt__", K2), ("__ge__", K3)]:
    got = [(K(a) < K(b), K(a) <= K(b), K(a) > K(b), K(a) >= K(b)) for a in (1, 2, 3) for b in (1, 2, 3)]
    print(root, got == [(a < b, a <= b, a > b, a >= b) for a in (1, 2, 3) for b in (1, 2, 3)])
    for n in ["__lt__", "__le__", "__gt__", "__ge__"]:
        if n != root:
            m = getattr(K, n)
            print(" ", n, m.__name__, m.__qualname__)
    try:
        K(1) < 5
        print("  no error")
    except TypeError as e:
        print("  TypeError", "not supported" in str(e))


@total_ordering
class A:
    def __init__(self, v):
        self.v = v

    def __lt__(self, o):
        return self.v < o.v

    def __eq__(self, o):
        return self.v == o.v


print(sorted([A(3), A(1), A(2)], key=lambda a: a, reverse=True)[0].v)
print(A(1) >= A(1), A(2) > A(1), A(1) <= A(0))
print(A.__ge__.__doc__)


# a subclass keeps its own root; the parent's derived methods stay
@total_ordering
class B(A):
    def __gt__(self, o):
        return self.v > o.v * 10


print(B(5) > B(1), B(20) > B(1), B(5) < B(6))

try:

    @total_ordering
    class N:
        pass

except ValueError as e:
    print("ValueError", e)


# exceptions from the root propagate
@total_ordering
class R:
    def __lt__(self, o):
        raise KeyError("boom")

    def __eq__(self, o):
        return False


try:
    R() >= R()
except KeyError as e:
    print("KeyError", e)
