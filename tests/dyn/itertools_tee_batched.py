import itertools, copy, gc
a, b = itertools.tee([1, 2, 3])
print(next(a), list(b), list(a), type(a).__name__)
c, d = itertools.tee(a)
print(c is a, list(c), list(d))
t1, t2 = itertools.tee(iter(range(5)))
next(t1); t3 = copy.copy(t1); print(list(t3), list(t1), list(t2))
x, = itertools.tee('ab', 1); print(list(x), itertools.tee('ab', 0))
class It:
    def __init__(s, i=0): s.i = i
    def __iter__(s): return s
    def __next__(s):
        s.i += 1
        if s.i > 3: raise StopIteration
        return s.i
    def __copy__(s): return It(s.i)
p, q = itertools.tee(It()); print(type(p).__name__, list(p), list(q))
print(list(itertools.batched('abcdefg', 3)), list(itertools.batched([], 2)))
try: list(itertools.batched('abc', 2, strict=True))
except ValueError as e: print(e)
try: itertools.batched('a', 0)
except ValueError as e: print(e)
print(type(itertools.batched('a', 1)).__name__)
# memory: lockstep consumption keeps nothing
u, v = itertools.tee(itertools.count())
for _ in range(200000):
    next(u); next(v)
print(next(u), next(v))
def g():
    yield from range(3)
m, n = itertools.tee(g()); del m; print(list(n))
try:
    it = None
    def gen():
        yield next(it)
    it, it2 = itertools.tee(gen())
    next(it)
except RuntimeError as e: print('RE', e)

# itertools._tee called directly: a copy of a tee, or a lone member over iter(x)
t1, t2 = itertools.tee("abc")
tnew = type(t1)
for args in [(), (10,), (1, 2)]:
    try:
        tnew(*args)
    except TypeError as e:
        print('TypeError', e)
t3 = tnew(t1)
print(list(t1), list(t2), list(t3))
t4 = tnew([1, 2, 3])
print(type(t4).__name__, list(t4))
