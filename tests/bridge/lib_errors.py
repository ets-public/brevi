# Exceptions from a library: matched by the library's classes and by builtin bases, with their attributes.
import brlib
from brlib import Store, LibError, NotFound


def lookup(s, i):
    try:
        return s.get(i)
    except NotFound as e:
        print("not found:", e, e.code, type(e).__name__, isinstance(e, KeyError), isinstance(e, LibError))
        return None


s = Store().add("a")
print(lookup(s, 0), lookup(s, 3))
try:
    s.get(7)
except KeyError as e:
    print("as KeyError:", e.args, str(e))
try:
    s.get(7)
except (ValueError, brlib.LibError) as e:
    print("tuple:", e.code)
try:
    brlib.Model.__init__(brlib.Model())
except Exception as e:
    print("other:", type(e).__name__)
with Store() as t:
    t.add(1)
    print("inside", len(t))
with Store() as t:
    raise ValueError("swallowed")
print("after")
try:
    with Store():
        raise KeyError("kept")
except KeyError as e:
    print("caught", e)


def boom(x):
    if x == 2:
        raise ValueError("bad two")
    return x


try:
    brlib.apply(boom, [1, 2, 3])
except ValueError as e:
    print("from callback:", e)


class Mine(Exception):
    pass


def boom2(x):
    raise Mine("mine")


try:
    brlib.apply(boom2, [1])
except Mine as e:
    print("own class back:", type(e).__name__, e)
