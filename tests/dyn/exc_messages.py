# Built-in exceptions keep their arguments; str.center pads like CPython; constant set literals print in CPython's order.
def g(x):
    raise ValueError(f"bad {x}")
try:
    g(1)
except ValueError as e:
    print("A", e, e.args)
try:
    raise RuntimeError("x")
except (TypeError, RuntimeError) as e:
    print("B", e, e.args, type(e).__name__)
def h():
    raise KeyError("k")
try:
    h()
except KeyError as e:
    print("C", e, repr(e))
print("x".center(4) + "|", "ab".center(5, "-"), "abc".center(8, "*"), "abcd".center(7))
print({10, 20, 30, 40, 100, 7}, {-1, -2, 5}, {99, 33, 64, 1, 2, 3, 4, 5, 200}, {(1, 2), (3, 4), (0, 0)})
