import functools
@functools.lru_cache(maxsize=2)
def f(x):
    print("miss", x)
    return x * 10
for x in [1, 2, 1, 3, 2, 1, 1]:
    print(f(x))
print(f.cache_info())
f.cache_clear()
print(f(1), f.cache_info())
@functools.lru_cache(maxsize=None)
def g(x):
    return x
print(g(1), g(1), g.cache_info())
# a key whose __eq__ calls the cached function again: a hit moves a result to most recently used without
# changing the cache's dict, so the outer lookup goes on (CPython keeps the order in a linked list)
@functools.lru_cache(maxsize=4)
def h(k):
    return k.n
class Key:
    def __init__(self, n):
        self.n = n
    def __hash__(self):
        return self.n
    def __eq__(self, other):
        if self.n == 2:
            h(Key(1))
        return self.n == other.n
h(Key(1)); h(Key(2))
print(h(Key(2)), h.cache_info())
