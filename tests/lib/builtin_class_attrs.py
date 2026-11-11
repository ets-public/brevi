# A builtin or a library module's function held as a class attribute does not bind (it is no descriptor in
# CPython); functools._CacheInfo is the one class every cache_info() returns.
import functools


class T:
    reduce = functools.reduce
    partial = functools.partial
    ck = functools.cmp_to_key
    ln = len
    mx = max


t = T()
print(t.reduce(lambda a, b: a + b, [1, 2, 3]), T.reduce(lambda a, b: a * b, [2, 3]))
print(t.partial(max, 5)(3))
print(t.ck(lambda a, b: a - b)(1) < t.ck(lambda a, b: a - b)(2))
print(t.ln([1, 2]), t.mx(3, 9))


@functools.lru_cache(maxsize=2)
def sq(x):
    return x * x


sq(2), sq(2), sq(3)
info = sq.cache_info()
print(info, type(info) is functools._CacheInfo, functools._CacheInfo(1, 2, 3, 4))
print(functools._CacheInfo.__name__, functools._CacheInfo._fields)
import functools
k = functools.cmp_to_key(mycmp=lambda x, y: y - x)
o = k(10)
print(o.obj, sorted([1, 3, 2], key=k), k(obj=4).obj)
for f in (lambda: functools.cmp_to_key(), lambda: k(), lambda: k(1, 2), lambda: k(1, obj=2), lambda: functools.cmp_to_key(f=1)):
    try:
        f()
    except TypeError as e:
        print(e)
