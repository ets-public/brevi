import warnings; warnings.simplefilter("ignore", DeprecationWarning)
import itertools, pickle, copy
a, b = itertools.tee(iter([1, 2, 3, 4]))
print(next(a), next(a))
c = copy.copy(a)
print(list(c), list(a), list(b))
a, b = itertools.tee(range(5))
next(a)
d = copy.deepcopy(a)
print(list(d), next(a))
a, b = itertools.tee('abcdef')
next(a); next(a)
for proto in range(pickle.HIGHEST_PROTOCOL + 1):
    a2 = pickle.loads(pickle.dumps(a, proto))
    print(proto, list(a2))
print(list(a), list(b))
# A long run crosses the 57-item link boundary.
a, b = itertools.tee(range(200))
for _ in range(130): next(a)
print(next(a), next(b), list(pickle.loads(pickle.dumps(a)))[:3])
print(type(a).__name__)
t = itertools.tee([1])[0]
for bad in [None, (1,), (1, 2), ('x', 1)]:
    try:
        t.__setstate__(bad)
        print('accepted', bad)
    except (TypeError, ValueError) as e:
        print(type(e).__name__)
x, y, z = itertools.tee([1, 2], 3)
print(list(x), list(y), list(z), itertools.tee([1], 0))
try:
    itertools.tee([1], -1)
except ValueError as e:
    print(e)
