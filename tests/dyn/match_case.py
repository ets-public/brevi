# match statements (PEP 634): every kind of pattern, CPython's order of checks, and names bound only on a match
import collections
import collections.abc
from dataclasses import dataclass


@dataclass
class Point:
    x: int
    y: int


class Plain:
    def __init__(self, a):
        self.a = a

    def __repr__(self):
        return f"Plain({self.a!r})"


def kind(v):
    match v:
        case None:
            return "none"
        case True:
            return "true"
        case 0 | 1:
            return "small"
        case -3 | 2.5 | 1 + 2j:
            return "literal"
        case "spam" "eggs" | b"raw":
            return "string"
        case [] | ():
            return "empty"
        case [9, *rest]:
            return f"nine then {rest}"
        case (a, b, *_):
            return f"two or more: {a} {b}"
        case [x]:
            return f"one: {x}"
        case {"k": k, **others}:
            return f"map k={k} others={others}"
        case {}:
            return "other map"
        case Point(x=0, y=yy) | Point(0, yy):
            return f"on the y axis at {yy}"
        case Point(px, py) as p if px == py:
            return f"diagonal {p}"
        case Point():
            return "a point"
        case Plain(a=[_, _] as pair):
            return f"plain pair {pair}"
        case int(n) if n > 100:
            return f"big {n}"
        case str() | bytes():
            return "text"
        case _:
            return "other"


for v in [None, True, False, 0, 1, -3, 2.5, 1 + 2j, "spameggs", b"raw", [], (), [9, 8, 7], (3, 4, 5), [6],
          {"k": 1, "z": 2}, {"q": 1}, collections.OrderedDict(k=5), Point(0, 4), Point(3, 3), Point(1, 2),
          Plain([1, 2]), Plain(3), 500, 50, "s", b"b", 7.5, range(2), collections.deque([4, 5])]:
    print(repr(v), "->", kind(v))

# a pattern that fails part way binds nothing
match (1, 2):
    case (x, 3):
        pass
    case _:
        pass
try:
    print(x)
except NameError as e:
    print("NameError:", e)

# a failing guard still leaves the pattern's names bound
match [5, 6]:
    case [first, second] if first > second:
        print("no")
    case _:
        print("guard failed, first =", first, "second =", second)

# __match_args__ and its errors
class Two:
    __match_args__ = ("a", "b")

    def __init__(self):
        self.a = 1
        self.b = 2


class Bad:
    __match_args__ = ["a"]


def attempt(f):
    try:
        print(f())
    except (TypeError, ValueError) as e:
        print(type(e).__name__ + ":", e)


def two_pos():
    match Two():
        case Two(p, q):
            return (p, q)


def too_many():
    match Two():
        case Two(p, q, r):
            return "no"


def bad_args():
    match Bad():
        case Bad(z):
            return "no"


def not_a_class():
    match 1:
        case max(0):
            return "no"


def repeated():
    match Two():
        case Two(p, a=q):
            return "no"


class Keys:
    KEY = "a"


def dup_key():
    match {"a": 0, "b": 1}:
        case {Keys.KEY: y, "a": z}:
            return "no"


for f in [two_pos, too_many, bad_args, not_a_class, repeated, dup_key]:
    attempt(f)

# sequence and mapping flags: registration, ABCs and multiple inheritance
class Reg:
    pass


collections.abc.Sequence.register(Reg)


class RegChild(Reg):
    pass


class M4(dict, collections.abc.Sequence):
    pass


class S1(collections.UserList, collections.abc.Mapping):
    pass


def shape(v):
    match v:
        case [*_]:
            return "seq"
        case {}:
            return "map"
        case _:
            return "neither"


for v in [Reg(), RegChild(), M4(), S1(), "abc", b"abc", bytearray(b"x"), {1, 2}, collections.UserDict()]:
    print(type(v).__name__, shape(v))

# the subject is evaluated once; a star wildcard reads by index
class Seq(collections.abc.Sequence):
    def __init__(self):
        self.log = []

    def __len__(self):
        self.log.append("len")
        return 4

    def __getitem__(self, i):
        self.log.append(i)
        if i >= 4:
            raise IndexError(i)
        return i * 10


s = Seq()
match s:
    case [a, *_, d]:
        print(a, d, s.log)
s = Seq()
match s:
    case [a, *mid]:
        print(a, mid, s.log)


def counted():
    counted.n += 1
    return [1, 2]


counted.n = 0
match counted():
    case [1, 3]:
        pass
    case [1, 2]:
        print("matched, subject evaluated", counted.n, "time")


# a match statement's own names never show in locals()
def local_names(v):
    match v:
        case [a, b]:
            return sorted(locals())


print(local_names([1, 2]))
