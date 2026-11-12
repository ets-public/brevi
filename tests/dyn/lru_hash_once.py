# lru_cache hashes each call's key once: lookup, insert and eviction reuse it
import functools
class K:
    def __init__(self, n): self.n = 0; self.v = n
    def __hash__(self):
        self.n += 1
        return 999
    def __eq__(self, o): return isinstance(o, K) and self.v == o.v
for ms in (1, 2, None, 0):
    @functools.lru_cache(maxsize=ms)
    def f(x, y): return y
    a, b = K(1), K(2)
    f(a, 1); print(ms, a.n)
    f(a, 1); print(ms, a.n)
    f(b, 2); print(ms, a.n, b.n)
    f(a, 1); print(ms, a.n, b.n)
    f(x=b, y=3); print(ms, b.n)
    print(f.cache_info())
@functools.lru_cache(typed=True)
def g(x): return x
k = K(3); g(k); g(k); print(k.n)
