# pickle.dumps memoizes by object identity, so the bytes say which str, bytes, tuple and complex values are one
# object: as in CPython, equal constants of a module are one object (the compiler's constant cache, including
# what it folds: "a" + "b", (1, 2) * 2), runtime results are new ones, and operations that CPython answers with
# the same object (s[:], s + "", "".join([s]), f"{s}") keep it. A defaultdict and a Counter pickle as CPython's,
# and load back with their default_factory.
import collections
import pickle


def f():
    return "abc"


def g():
    return (1, 2)


s = "hello world"
t = ("x y", 1)
cases = [
    [s, s],
    ["abc", "abc", f(), "ab" + "c"],
    ["hello world", s[:], s + "", "" + s, "".join([s]), f"{s}", str(s), s * 1, "%s" % s, "{}".format(s)],
    [s.lower(), s.lower(), "-".join(["a", "b"]), "a-b"],
    [(1, 2), g(), (1, 2), (0, 1 + 1), ((1, 2),)],
    [t, t[:], (t,), (t,), t + ()],
    [b"xy", b"xy", b"x" + b"y", b"xy"[:]],
    [1j, 1j, (2j, 2j)],
    ["x" * 3, "xxx", ("a",) * 2, ("a", "a")],
    {"key one": 1, "k2": ["key one", "k2"]},
    [s.split("q"), s.split()],
]
for c in cases:
    for p in range(pickle.HIGHEST_PROTOCOL + 1):
        print(p, pickle.dumps(c, p))
x = pickle.loads(pickle.dumps([s, s, (1, 2), (1, 2)]))
print(x[0] is x[1], x[2] is x[3])


def mk():
    return 7


for v in [collections.defaultdict(list, {"a": [1]}), collections.defaultdict(int), collections.defaultdict(),
          collections.defaultdict(mk, {1: 2}), collections.Counter("abca"), [collections.defaultdict(list)] * 2]:
    for p in range(pickle.HIGHEST_PROTOCOL + 1):
        print(p, pickle.dumps(v, p))
d = pickle.loads(pickle.dumps(collections.defaultdict(list)))
d["z"].append(1)
print(type(d).__name__, d["z"], d.default_factory is list)
c = pickle.loads(pickle.dumps(collections.Counter("aab")))
print(type(c).__name__, c["q"], c.most_common(1))
e = pickle.loads(pickle.dumps(collections.defaultdict(mk)))
print(e[0], e.default_factory is mk)
