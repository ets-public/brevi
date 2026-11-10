import gc, weakref
class N:
    def __init__(self, v): self.v = v; self.peer = None
# reachable cycle, referenced from a global only through a closure and a list
a = N(1); b = N(2); a.peer = b; b.peer = a
keep = [a]
def mk(bb):
    def getter(): return bb
    return getter
getter = mk(b)
ra, rb = weakref.ref(a), weakref.ref(b)
del a, b
gc.collect()
print(ra() is not None, rb() is not None, getter().peer.v, keep[0].peer.v)
# unreachable cycle through a dict and a list
x = N(3); x.peer = {"me": [x]}
rx = weakref.ref(x, lambda r: print("callback"))
del x
print(gc.collect() > 0, rx())
# reachable only from a generator frame
def gen():
    n = N(4); n.peer = n
    r = weakref.ref(n)
    yield r
    yield n.v
g = gen(); r = next(g)
gc.collect()
print(r() is not None, next(g))
# reachable through a bound method
class M:
    def m(self): return self
o = M(); o.self_ref = o; bm = o.m; ro = weakref.ref(o); del o
gc.collect()
print(ro() is not None, bm() is ro())
try:
    weakref.ref(5)
except TypeError as e: print("TE", e)
