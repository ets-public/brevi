# itertools iterators (and map, filter, zip, enumerate) pickle, copy and deep-copy as in CPython 3.13: the
# reduce tuple names the iterator's class, a pickle loads back as the same type, copies are independent, and
# every itertools __reduce__/__setstate__ issues the 3.13 DeprecationWarning at the frame CPython names.
import copy
import itertools as it
import operator
import os
import pickle
import warnings

warnings.simplefilter("ignore", DeprecationWarning)


def even(x):
    return x % 2 == 0


def key(x):
    return x // 3


def roundtrips(make, n=4):
    out = []
    for take in (0, 1, 3):
        x = make()
        for _ in range(take):
            next(x, None)
        got = [list(it.islice(pickle.loads(pickle.dumps(x, p)), n)) for p in range(pickle.HIGHEST_PROTOCOL + 1)]
        got.append(list(it.islice(copy.copy(x), n)))
        got.append(list(it.islice(copy.deepcopy(x), n)))
        y = pickle.loads(pickle.dumps(x))
        out.append((type(y) is type(x), all(g == got[0] for g in got), got[0]))
    return out


cases = {
    "chain": lambda: it.chain("abc", [1, 2], ()),
    "from_iterable": lambda: it.chain.from_iterable(["ab", "cd"]),
    "cycle": lambda: it.cycle("abc"),
    "count": lambda: it.count(5),
    "count step": lambda: it.count(1.5, 0.5),
    "repeat": lambda: it.repeat("x", 3),
    "islice": lambda: it.islice(range(20), 2, 15, 3),
    "accumulate": lambda: it.accumulate(range(6)),
    "accumulate initial": lambda: it.accumulate([1, 2, 3], operator.mul, initial=10),
    "starmap": lambda: it.starmap(operator.pow, [(2, 3), (3, 2), (4, 1)]),
    "takewhile": lambda: it.takewhile(even, [2, 4, 5, 6]),
    "dropwhile": lambda: it.dropwhile(even, [2, 4, 5, 6]),
    "compress": lambda: it.compress("abcdef", [1, 0, 1, 0, 1, 1]),
    "filterfalse": lambda: it.filterfalse(even, range(9)),
    "zip_longest": lambda: it.zip_longest("abc", "d", fillvalue="-"),
    "groupby": lambda: it.groupby(range(9), key),
    "map": lambda: map(even, range(5)),
    "filter": lambda: filter(even, range(9)),
    "zip": lambda: zip("abc", range(3)),
    "enumerate": lambda: enumerate("abc", 10),
    "list_iterator": lambda: iter([1, 2, 3, 4, 5]),
    "range_iterator": lambda: iter(range(3, 9)),
    "product": lambda: it.product("ab", range(3), repeat=2),
    "combinations": lambda: it.combinations("abcde", 3),
    "cwr": lambda: it.combinations_with_replacement("abc", 2),
    "permutations": lambda: it.permutations("abcd", 2),
}
for name, make in cases.items():
    if name == "groupby":
        rows = []
        for take in (0, 1, 2):
            g = make()
            for _ in range(take):
                next(g)
            rows.append([(k, list(v)) for k, v in pickle.loads(pickle.dumps(g))])
        print(name, rows)
        continue
    print(name, roundtrips(make))

# the reduce tuples
c = it.chain("ab", "cd")
next(c)
r = c.__reduce__()
print(r[0], r[1], [type(x).__name__ for x in r[2]])
cy = it.cycle("ab")
next(cy)
print(cy.__reduce__()[0], cy.__reduce__()[2])
print(it.count(3).__reduce__(), it.count(2.0, 3).__reduce__(), repr(it.count(2**70, 3)))
print(it.repeat(7, 2).__reduce__(), it.repeat(7).__reduce__())
s = it.islice("abcdef", 1, None, 2)
next(s)
print(s.__reduce__()[0], s.__reduce__()[1][1:], s.__reduce__()[2])
print(zip("a", "b", strict=True).__reduce__()[2], enumerate("a", 4).__reduce__()[1][1])
print(it.accumulate([None, None], lambda a, b: a).__reduce__()[0])
a = it.accumulate([None, None, 3], lambda a, b: a)
next(a)
print(a.__reduce__()[0], list(copy.deepcopy(a)))

p = it.permutations("abc")
next(p)
next(p)
print(p.__reduce__(), it.combinations("ab", 3).__reduce__(), it.product("ab", []).__reduce__())

# __setstate__
ch = it.chain()
ch.__setstate__((iter(["abc", "de"]), iter("xy")))
print(list(ch))
cy = it.cycle("defg")
cy.__setstate__((list("abc"), 0))
print(list(it.islice(cy, 9)))
for bad in [(), [], 0, ([],), (iter([]), [])]:
    try:
        it.chain().__setstate__(bad)
    except TypeError as e:
        print("TypeError", e)
for bad in [[list("ab"), 0], (tuple("ab"), 0), (list("ab"), "x"), ()]:
    try:
        it.cycle("ab").__setstate__(bad)
    except TypeError as e:
        print("TypeError", e)
li = iter([1, 2, 3, 4])
li.__setstate__(2)
print(list(li))
ri = iter(range(10))
ri.__setstate__(7)
print(list(ri))

# copies are independent of the original
src = iter([1, 2, 3, 4])
cp = copy.copy(src)
next(cp)
print(next(src), list(cp))
z = zip([1, 2], [3, 4])
zc = copy.deepcopy(z)
print(list(z), list(zc))

# generators do not pickle or copy; pairwise and batched have no __reduce__
for x in [(i for i in range(3)), it.pairwise("abc"), it.batched("abc", 2)]:
    for f in (copy.copy, pickle.dumps):
        try:
            f(x)
        except TypeError as e:
            print("TypeError", e)

# a pickle CPython made of an itertools.cycle loads as the program's own
data = b"\x80\x04\x95=\x00\x00\x00\x00\x00\x00\x00\x8c\titertools\x8c\x05cycle\x93\x8c\x08builtins\x8c\x04iter\x93](K\x01K\x02K\x03e\x85RK\x01b\x85R]K\x01aK\x00\x86b."
cy = pickle.loads(data)
print(type(cy), list(it.islice(cy, 7)))
print(pickle.loads(pickle.dumps(operator.add)) is operator.add)

# the DeprecationWarnings: pickle names the program's line, copy names copy.py's
with warnings.catch_warnings(record=True) as w:
    warnings.simplefilter("always")
    pickle.dumps(it.count(1))
    copy.copy(it.cycle("ab"))
    copy.deepcopy(it.chain("a"))
    pickle.dumps(map(even, [1]))
    it.cycle("ab").__setstate__((["x"], 1))
for x in w:
    print(x.category.__name__, os.path.basename(x.filename), x.lineno if x.filename == __file__ else "copy.py", x.message)
with warnings.catch_warnings():
    warnings.simplefilter("error", DeprecationWarning)
    for f in (lambda: pickle.dumps(it.islice("ab", 1)), lambda: copy.copy(it.count()), lambda: it.chain().__reduce__()):
        try:
            f()
        except DeprecationWarning as e:
            print("raised", e)

# constructor arguments are checked as CPython's argument clinic does
calls = [
    lambda: it.permutations("abc", 2, 1), lambda: it.permutations(iterable="ab", r=1), lambda: it.permutations("ab", x=1),
    lambda: it.groupby("abc", lambda x: x, 10), lambda: it.groupby("ab", iterable="x"), lambda: it.combinations(iterable="abc", r=2),
    lambda: it.combinations("abc"), lambda: it.combinations_with_replacement(r=1, iterable="ab"), lambda: it.accumulate([1, 2], None, 3),
    lambda: it.accumulate([1], initial=2), lambda: it.accumulate([1], foo=2), lambda: it.count(1, 2, 3), lambda: it.count(start=3),
    lambda: it.compress(data="ab", selectors=[1, 0]), lambda: it.product("ab", repeat=2, x=1), lambda: it.product("a", x=1),
    lambda: it.zip_longest("a", fill=1), lambda: it.starmap(f=1), lambda: it.takewhile(bool, [1], x=1), lambda: it.cycle("a", x=1),
]
for c in calls:
    try:
        print(list(it.islice(c(), 4)))
    except TypeError as e:
        print("TypeError", e)
