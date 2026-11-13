# deque pickling (CPython's deque.__reduce__ form), deque subclass operators and str
import pickle
from collections import deque
class Deque(deque):
    pass
class DequeWithSlots(deque):
    __slots__ = ('x', 'y', '__dict__')
for d in deque(range(5)), deque(range(5), 3):
    for proto in range(pickle.HIGHEST_PROTOCOL + 1):
        e = pickle.loads(pickle.dumps(d, proto))
        print(proto, e is d, type(e).__name__, list(e), e.maxlen)
orig = deque(range(6))
data = [i * 1.01 for i in orig]
for proto in range(pickle.HIGHEST_PROTOCOL + 1):
    itorg = iter(orig)
    next(itorg)
    it, d = pickle.loads(pickle.dumps((itorg, orig), proto))
    for i, x in enumerate(data):
        d[i] = x
    print(type(it) == type(itorg), list(it))
for cls in Deque, DequeWithSlots:
    for d in cls('abc'), cls('abcde', maxlen=4):
        d.x = ['x']
        d.z = ['z']
        for proto in range(pickle.HIGHEST_PROTOCOL + 1):
            e = pickle.loads(pickle.dumps(d, proto))
            print(type(e).__name__, list(e), e.maxlen, e.x, e.z, hasattr(e, 'y'), e is d)
d = deque('abc'); d.append(d)
e = pickle.loads(pickle.dumps(d))
print(e[-1] is e, e.maxlen)
d = Deque('abc', 3); d.append(d)
e = pickle.loads(pickle.dumps(d)); print(type(e).__name__, e[-1] is e)
d.x = d
e = pickle.loads(pickle.dumps(d)); print(e.x is e)
import copy
it = iter(deque([1,2,3])); next(it)
print(list(copy.copy(it)), list(copy.deepcopy(it)), list(it))
print(iter(deque([1,2])).__reduce__())
class S2(deque):
    def __init__(self, *a):
        print("init", len(a))
        super().__init__(*a)
u = S2([0, 1], 3)
for r in (u * 1, 2 * u, u * 0, u + u, u + deque([5])):
    print(type(r).__name__, r, r.maxlen, r is u)
try:
    u + [1]
except TypeError as e:
    print(e)
d = deque([1]) + u
print(type(d).__name__, d)
class R(S2):
    def __repr__(self):
        return "R!"
print(str(Deque([1])), f"{Deque([2])}", R([1]), str(R()))
