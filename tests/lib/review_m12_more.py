def show(**kw): return sorted(kw.items())
def g(x, y=2, *a, **k): return (x, y, a, k)
def t(f, *a, **k):
    try: print(f(*a, **k))
    except Exception as e: print(type(e).__name__, e)
t(lambda: show(**{"x":1}, x=9))
a={'x':1,'y':2}; b={'y':3,'x':4}
t(lambda: show(**a, **b))
t(lambda: g(1, x=2))
t(lambda: g(1, **{'x':2}))
t(lambda: g(**a, **b))
t(lambda: g(y=1, **{'y':2}))
t(lambda: g(1,2,3,x=1,**{'x':2}))
t(lambda: dict(**a, y=5))
t(lambda: dict(a, **b))
t(lambda: dict(x=1, **{'x':2}))
t(lambda: sorted([3,1], key=abs, **{'key':abs}))
t(lambda: print(1, sep='', **{'sep':''}))
class C:
    def m(self, **k): return k
    @staticmethod
    def s(**k): return k
    @classmethod
    def c(cls, **k): return k
    def __init__(self, **k): self.k=k
c=C()
t(lambda: c.m(**{'a':1}, a=2))
t(lambda: C.s(a=1, **{'a':2}))
t(lambda: C.c(a=1, **{'a':2}))
t(lambda: C(a=1, **{'a':2}))
t(lambda: C.m(c, a=1, **{'a':2}))
def outer():
    def inner(**k): return k
    class D:
        def f(self, **k): return k
    return inner, D
i, D = outer()
t(lambda: i(a=1, **{'a':2}))
t(lambda: D().f(a=1, **{'a':2}))
t(lambda: (lambda **k: k)(a=1, **{'a':2}))
d={}
t(lambda: d.update(a=1, **{'a':2}))
t(lambda: d.get(**{'key':1}, key=1))
t(lambda: "x".format(a=1, **{'a':2}))
t(lambda: "{a}".format(**{'a':1}, a=2))
from collections import Counter, OrderedDict, defaultdict
t(lambda: Counter(a=1, **{'a':2}))
t(lambda: OrderedDict(a=1, **{'a':2}))
t(lambda: defaultdict(int, a=1, **{'a':2}))
import json, math
t(lambda: json.dumps({}, indent=1, **{'indent':2}))
t(lambda: math.log(1, base=2, **{'base':2}))
t(lambda: int("1", base=2, **{'base':3}))
t(lambda: str(object=1, **{'object':2}))
t(lambda: max([1], key=None, **{'key':None}))
import functools
t(lambda: functools.partial(g, x=1)(x=2))
t(lambda: functools.partial(g, 1, y=1)(**{'y':2}))
def gen(**k):
    yield k
t(lambda: gen(a=1, **{'a':2}))
t(lambda: g(**{'x':1}, **{'x':2}, **{'x':3}))
t(lambda: g(*[1,2], **{'y':2}))
t(lambda: g(**{1:2}))
t(lambda: show(**{'z':1}, **{'z':2}, q=1))
t(lambda: C.m(a=1, **{'a':2}))
