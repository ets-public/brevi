# functools.partial as CPython 3.13's C type
import functools, copy
from functools import partial

def f(*a, **k):
    return a, k

p = partial(f, 1, x=2)
print(type(p), type(p).__name__, type(p).__module__, isinstance(p, partial), callable(p))
print(p(3, y=4), p(x=5))
print(repr(p.func) == repr(f), p.args, p.keywords)
print(vars(p), p.__dict__)
p.attr = 7
print(vars(p), p.attr)
q = partial(p, 9)
print(q.func is p, q.args)
r = partial(partial(f, 1), 2)
print(r.func is f, r.args)
class S(partial):
    pass
sp = S(f, 1)
print(repr(sp).startswith("__main__.S(<function f"), isinstance(sp, partial), sp(2))
print(partial(S(f, 1), 2).args, S(partial(f, 1), 2).args)
for name in ("func", "args", "keywords"):
    try:
        setattr(p, name, 1)
    except AttributeError as e:
        print(name, e)
    try:
        delattr(p, name)
    except AttributeError as e:
        print("del", name, e)
try:
    del p.__dict__
except TypeError as e:
    print(e)
try:
    partial()
except TypeError as e:
    print(e)
try:
    partial(1)
except TypeError as e:
    print(e)
print(repr(partial.func), type(partial.func).__name__)
print(partial[int], S[int])
red = partial(max, 1, k=2).__reduce__()
print(red[0] is partial, red[1], red[2])
p2 = partial(max)
p2.__setstate__((min, (3, 5), None, None))
print(p2.func, p2.args, p2.keywords, p2())
p2.__setstate__((min, (3, 4), {"default": 0}, {"a": 1}))
print(vars(p2), p2.a)
print(sorted([3, 1, 2], key=partial(lambda m, x: -x * m, 1)))
print(list(map(partial(pow, 2), [1, 2, 3])))
class A:
    m = partial(f, "bound?")
print(A().m(1), A.m(2))
cp = copy.copy(partial(f, 1, z=3))
print(cp.args, cp.keywords)
pp = partial(f)
pp.keywords["n"] = 1
print(pp())
rec = partial(f)
rec.__setstate__((f, (rec,), {}, None))
print(repr(rec).count("..."))
print(hash(p) == hash(p), p == p, p != partial(f, 1, x=2))
print(partial.__doc__.splitlines()[0])
