import operator
from itertools import repeat, count, chain
class A:
    def __length_hint__(self): return 5
class B:
    def __length_hint__(self): return NotImplemented
class C:
    def __length_hint__(self): raise TypeError("x")
class D:
    def __length_hint__(self): return -1
class E:
    def __length_hint__(self): return "a"
class F:
    def __len__(self): return 3
    def __length_hint__(self): return 9
class G(list): pass
class H:
    def __length_hint__(self): raise ValueError("boom")
for o in [A(), B(), C(), F(), G([1,2]), [1,2,3], (1,), "abc", {1:2}, {1}, range(5), 5, None, map(abs,[1]), (x for x in []), count(), chain([1]), repeat(1,3), repeat(1), repeat(1,-4)]:
    print(operator.length_hint(o), operator.length_hint(o, 7))
it = iter([1,2,3]); next(it); print(operator.length_hint(it))
it = iter((1,2)); next(it); next(it); print(operator.length_hint(it)); list(it); print(operator.length_hint(it))
it = reversed([1,2,3]); next(it); print(operator.length_hint(it))
l=[1,2,3]; it=iter(l); l.append(4); print(operator.length_hint(it)); l.clear(); print(operator.length_hint(it))
r = repeat('a', 2); next(r); print(operator.length_hint(r)); list(r); print(operator.length_hint(r))
for o in [D(), E(), H()]:
    try: print(operator.length_hint(o))
    except Exception as e: print(type(e).__name__, e)
for d in ['a', 1.5, None]:
    try: print(operator.length_hint([], d))
    except Exception as e: print(type(e).__name__, e)
print(operator.length_hint([], True))
try: operator.length_hint(A(), default=2)
except TypeError as e: print(e)
