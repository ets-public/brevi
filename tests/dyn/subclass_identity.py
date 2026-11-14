# A str, bytes or tuple subclass instance: where the exact type answers with the object itself (s[:], s + "",
# s.strip()), CPython's answer for the subclass is a new exact object, never the value it was made from, and
# partition gives back the instance itself. pickle memoizes by identity, so its bytes show the same.
import copy
import pickle
import sys


class S(str):
    pass


class B(bytes):
    pass


class T(tuple):
    pass


s, b, t = "hello world", b"hello", (1, 2, 3)
x, y, z = S(s), B(b), T(t)


def chk(name, f, base, me):
    r1, r2 = f(), f()
    print(name, type(r1).__name__, r1 is base, r1 is r2, r1 is me)


for name, f, base, me in [
    ("s[:]", lambda: x[:], s, x), ("b[:]", lambda: y[:], b, y), ("t[:]", lambda: z[:], t, z),
    ("s+''", lambda: x + "", s, x), ("''+s", lambda: "" + x, s, x), ("s*1", lambda: x * 1, s, x),
    ("t+()", lambda: z + (), t, z), ("()+t", lambda: () + z, t, z), ("t*1", lambda: z * 1, t, z),
    ("str()", lambda: str(x), s, x), ("bytes()", lambda: bytes(y), b, y), ("tuple()", lambda: tuple(z), t, z),
    ("replace", lambda: x.replace("q", "r"), s, x), ("strip", lambda: x.strip(), s, x),
    ("bound strip", lambda: x.strip(), s, x),
    ("lower", lambda: x.lower(), s, x), ("expandtabs", lambda: x.expandtabs(), s, x),
    ("format", lambda: "{}".format(x), s, x), ("%s", lambda: "%s" % x, s, x), ("f", lambda: f"{x}", s, x),
    ("join", lambda: "".join([x]), s, x), ("ljust", lambda: x.ljust(2), s, x), ("center", lambda: x.center(1), s, x),
    ("zfill", lambda: x.zfill(1), s, x), ("removeprefix", lambda: x.removeprefix("q"), s, x),
    ("b.replace", lambda: y.replace(b"q", b"r"), b, y), ("b.strip", lambda: y.strip(), b, y),
    ("b.ljust", lambda: y.ljust(1), b, y), ("b+''", lambda: y + b"", b, y), ("b*1", lambda: y * 1, b, y),
    ("split0", lambda: x.split("q")[0], s, x), ("splitlines0", lambda: x.splitlines()[0], s, x),
    ("partition0", lambda: x.partition("q")[0], s, x), ("rpartition2", lambda: x.rpartition("q")[2], s, x),
    ("b.split0", lambda: y.split(b"q")[0], b, y), ("b.partition0", lambda: y.partition(b"q")[0], b, y),
    ("copy", lambda: copy.copy(x), s, x), ("deepcopy t", lambda: copy.deepcopy(z), t, z),
    ("str.__str__", lambda: str.__str__(x), s, x),
]:
    chk(name, f, base, x if me is x else me)

for p in range(6):
    print(p, pickle.dumps([x[:], x[:], s], p), pickle.dumps([z[:], z[:], t], p), pickle.dumps([y[:], y[:], b], p))

# sys.intern: one str per text, which is a name-like constant of the program when there is one
a = "".join(["ab", "c"])
abc = "abc"
print(sys.intern(a) is abc, sys.intern(a) is sys.intern("".join(["a", "bc"])))
w = "".join(["not a", " name"])
print(sys.intern(w) is w, sys.intern("".join(["not a", " name"])) is w)
for bad in [lambda: sys.intern(1), lambda: sys.intern(x), lambda: sys.intern(), lambda: sys.intern("a", "b"), lambda: sys.intern(string="a")]:
    try:
        bad()
    except TypeError as e:
        print("TypeError", e)
