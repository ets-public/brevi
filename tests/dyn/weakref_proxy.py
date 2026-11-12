# weakref.proxy: every operation goes to the referent while it lives, ReferenceError after (expected = CPython 3.13)
import weakref, re, gc
class A:
    def __init__(s): s.v=3
    def __len__(s): return 5
    def __add__(s,o): return ('add',o)
    def __radd__(s,o): return ('radd',o)
    def __call__(s,*a): return a
    def __bool__(s): return False
    def __iter__(s): return iter([1,2])
    def __getitem__(s,i): return i*2
    def __setitem__(s,i,v): print('set', i, v)
    def __delitem__(s,i): print('del', i)
    def __eq__(s,o): return 'eq'
    def __lt__(s,o): return 'lt'
    def __str__(s): return 'STR'
    def __repr__(s): return 'REPR'
    def __contains__(s,x): return True
    def __neg__(s): return 'neg'
    def __hash__(s): return 1
    def __abs__(s): return 'abs'
class B:
    def m(self, x): return x + 1
a=A(); p=weakref.proxy(a)
print(type(p), type(weakref.proxy(B())), type(weakref.ref(a)))
print(len(p), p+1, 1+p, p(1,2), bool(p), list(p), p[3], p==1, p<1, str(p), 3 in p, -p, f"{p}", abs(p))
p[1] = 2; del p[4]
if p: print('truthy')
else: print('falsy')
tests = [lambda: format(p,"x"), lambda: float(p), lambda: int(p), lambda: reversed(p), lambda: round(p), lambda: range(p), lambda: dict(p), lambda: sorted(p), lambda: sum(p), lambda: '-'.join(map(str,p)), lambda: p.__len__(), lambda: [1,2,3][p:], lambda: [1,2,3][p], lambda: p*2, lambda: p.__index__(),
  lambda: hash(p), lambda: [1,2][p], lambda: hex(p), lambda: p is a, lambda: isinstance(p, A), lambda: isinstance(p, type(p)), lambda: isinstance(p, (int, A)),
  lambda: p.__class__, lambda: p.__dict__, lambda: p.v, lambda: weakref.ref(p), lambda: weakref.proxy(p), lambda: callable(p), lambda: p == p, lambda: p != 1,
  lambda: {p: 1}, lambda: repr(p)[:10], lambda: type(p).__name__]
for f in tests:
    try: print(repr(f()))
    except Exception as e: print(type(e).__name__, e)
p.w = 4; print(a.w); del p.w; print(hasattr(a,'w'))
b = B(); q = weakref.proxy(b)
print(callable(q), q.m(1), bool(q))
try: q()
except TypeError as e: print(e)
x = re.compile('a+'); print(weakref.ref(x)() is x, weakref.proxy(x).findall('aaba'))
try: weakref.proxy(x.match('a'))
except TypeError as e: print(e)
try: weakref.proxy(1)
except TypeError as e: print(e)
s = {1, 2}; ps = weakref.proxy(s); print(len(ps), 1 in ps, sorted(ps))
del a
for f in [lambda: p.v, lambda: bool(p), lambda: len(p), lambda: str(p), lambda: p + 1, lambda: 1 + p, lambda: p[0], lambda: list(p), lambda: p == 1, lambda: p(), lambda: isinstance(p, A), lambda: 1 in p, lambda: type(p).__name__, lambda: repr(p)[:15], lambda: hash(p)]:
    try: print(repr(f()))
    except Exception as e: print(type(e).__name__, e)
del b
try: q.m(1)
except ReferenceError as e: print('dead', e)
print(issubclass(ReferenceError, Exception), ReferenceError.__name__)

