# functools.partial pickles with CPython's bytes: partial.__reduce__'s (cls, (func,), (func, args, kw, dict)),
# the class and the function by name, memoized by identity; it loads back as the program's own partial.
# A module's function is named by CPython's own module for it (operator.add is _operator.add).
import functools
import json
import math
import operator
import os
import pickle


def f(*a, **k):
    return (a, k)


class P(functools.partial):
    pass


p1 = functools.partial(f, 1, b=2)
p3 = P(f, 5)
p3.x = 7
me = functools.partial(f, "kw", key="v", other=[1, 2])
me.me = me
cases = {
    "plain": p1,
    "empty": functools.partial(f),
    "subclass": p3,
    "flattened": functools.partial(p1, 9),
    "twice": [p1, p1, f],
    "self in dict": me,
    "subclass of partial": P(functools.partial(f, 1), 2),
    "in a dict": {"a": functools.partial(f), "t": ("x", "x")},
    "str args": functools.partial(f, "hello world", "hello world"),
    "operator": functools.partial(operator.add, 1),
}
for name, v in cases.items():
    for proto in range(6):
        d = pickle.dumps(v, proto)
        print(name, proto, d)
        r = pickle.loads(d)
        if name == "twice":
            print(r[0] is r[1], r[2] is f, r[0]())
        elif name == "in a dict":
            print(r["a"](), r["t"][0] is r["t"][1])
        elif name == "self in dict":
            print(r.me is r, r.args, r.keywords, r())
        elif name == "operator":
            print(r.func is operator.add, r(2))
        else:
            print(type(r).__name__, type(r.func).__name__, r.func is f, r.args, r.keywords, r.__dict__, r())
for fn in (operator.add, operator.pow, math.sqrt, os.path.join, json.dumps, functools.reduce):
    for proto in (0, 2, 4):
        d = pickle.dumps(fn, proto)
        print(d, pickle.loads(d) is fn)
