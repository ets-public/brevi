# id() and `is` agree for every kind of value: two live values have the same id exactly when `x is y`.
# Ints and floats are plain values in the dynamic build, so each value is one object (the language allows it
# for immutable types); id() used to give every float the id 1, so id(1.5) == id(2.5) was True.
import math
x, y = 1.5, 2.5
print(id(x) == id(y), id(x) == id(x), x is x, x is y)
print(id(-0.0) == id(0.0), -0.0 is 0.0, id(1) == id(1.0), id(True) == id(1), id(0) == id(False))
nan = float("nan")
nan2 = float("nan")
print(nan is nan, id(nan) == id(nan), nan is nan2, id(nan) == id(nan2))
z = math.inf - math.inf
w = z
print(w is z, id(w) == id(z))
fs = [0.1 * k for k in range(50)]
print(len({id(v) for v in fs}) == len(set(fs)))
n = 1000
m = n
print(n is m, id(n) == id(m), id(1000) == id(1001))
big = 2 ** 62
print(id(big) == id(big + 1), id(-big) == id(big), big is big)
huge = 10 ** 30
h2 = huge
print(huge is h2, id(huge) == id(h2), id(huge) == id(huge + 1))
print(id(None) == id(None), None is None, id(None) == id(False))
e = ()
print(e is (), id(e) == id(tuple([])), id((1,)) == id(e))
c1 = chr(97)
print(c1 is "a", id(c1) == id("a"), id("a") == id("b"), id("") == id(str()))
c = 1 + 2j
d = c
print(c is d, id(c) == id(d))
bs = b"ab"
print(bs is bs, id(bs) == id(bs))
meth = [].append
print(meth is meth, id(meth) == id(meth))
r = range(3)
print(r is r, id(r) == id(r), r is range(3))
sl = slice(1, 2)
print(sl is sl, id(sl) == id(sl))
dk = {}.keys()
print(dk is dk, id(dk) == id(dk))
a, b = [1], [1]
print(a is b, id(a) == id(b), id(a) == id(a))
ids = [id(v) for v in (1, 1.0, True, None, "a", (), 2 ** 70, -1, 0.5)]
print(len(set(ids)) == len(ids), all(isinstance(i, int) for i in ids))
