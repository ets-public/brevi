import collections.abc as abc
from collections.abc import Mapping, Sequence, Set, Hashable, Iterable, Sized, Container, Reversible, Collection, MutableSequence, Iterator, Generator, Callable
import collections.abc as abc
from collections.abc import Mapping, Callable, Iterable, Iterator, Hashable, Sequence, MutableMapping, Sized
IntFn = Mapping[int, int]
M = Mapping[str, int]
print(IntFn, M)
def f(x: Mapping[str, int]) -> Iterable[int]: return x
print(isinstance({}, Mapping), isinstance([], Sequence), isinstance((), Hashable), isinstance([], Hashable), isinstance(iter([]), Iterator), isinstance((x for x in []), abc.Generator))
print(isinstance(len, Callable), isinstance(1, Callable), isinstance('a', Sequence), isinstance(b'', Sequence), isinstance({}.keys(), abc.Set), isinstance(range(3), Sequence))
class Q:
    def __eq__(s, o): return True
print(isinstance(Q(), Hashable), issubclass(Q, Hashable))
class D(Mapping):
    def __init__(s): s.d = {'a': 1}
    def __getitem__(s, k): return s.d[k]
    def __iter__(s): return iter(s.d)
    def __len__(s): return len(s.d)
d = D(); print(d['a'], 'a' in d, d.get('z'), list(d.items()), d == {'a': 1}, dict(d), isinstance(d, Mapping))
try: Mapping()
except TypeError as e: print(e)
class S:
    def __len__(s): return 0
print(isinstance(S(), Sized), issubclass(S, Sized))
class R: pass
Sequence.register(R); print(isinstance(R(), Sequence), issubclass(R, Iterable))
o = object(); print(o.__eq__(1), o.__eq__(o), o.__ne__(1), o.__lt__(1))
class P: pass
p = P(); print(p.__eq__(1), p.__lt__(p), p.__ne__(p))
def g():
    yield isinstance(it, Iterator)
it = g(); print(next(it))
import datetime, decimal
print(isinstance(datetime.date(2020,1,1), Hashable), isinstance(decimal.Decimal(1), Hashable))
class MM(MutableMapping):
    def __init__(s): s.d = {}
    def __getitem__(s, k): return s.d[k]
    def __setitem__(s, k, v): s.d[k] = v
    def __delitem__(s, k): del s.d[k]
    def __iter__(s): return iter(s.d)
    def __len__(s): return len(s.d)
m = MM(); m.update(a=1); m.setdefault('b', 2); print(m.pop('a'), m.popitem(), len(m), m.keys() | {'x'})
print(type(Mapping), Mapping.__module__, Mapping.__name__, Mapping.__mro__)

vals = [None, 1, 1.5, 'a', b'a', [1], (1,), {1: 2}, {1}, frozenset(), range(2), {}.keys(), {}.values(), {}.items(), len, Mapping, iter([]), (x for x in []), slice(1), ..., NotImplemented]
for v in vals:
    print(type(v).__name__, [isinstance(v, A) for A in (Mapping, Sequence, Set, Hashable, Iterable, Sized, Container, Reversible, Collection, MutableSequence, Callable)])
print(type(Mapping).__name__, isinstance(Mapping, type), issubclass(type(Mapping), type))
class A2(Sequence):
    def __getitem__(s, i): return [1, 2, 3][i]
    def __len__(s): return 3
a = A2(); print(list(a), 2 in a, list(reversed(a)), a.index(2), a.count(1), isinstance(a, Reversible))
print(Mapping[str, int], Sequence[int], A2[int])
class H:
    def __eq__(s, o): return False
class H2(H):
    def __hash__(s): return 1
print(H.__hash__, isinstance(H(), Hashable), isinstance(H2(), Hashable), hash(H2()))
try: hash(H())
except TypeError as e: print(e)
class B:
    __hash__ = lambda s: 3
class C(B):
    def __eq__(s, o): return True
try: hash(C())
except TypeError as e: print(e)

# reversed() dict iterators pickle as CPython's: iter(list of the rest)
import pickle
_d = {1: 2, 3: 4, 5: 6}
for _it in (reversed(_d.items()), reversed(_d), reversed(_d.values()), reversed(_d.keys())):
    next(_it)
    print(list(pickle.loads(pickle.dumps(_it))), list(_it))

# cmp_to_key's K objects: __hash__ is None, __call__ is defined
import functools as _ft
_k = _ft.cmp_to_key(lambda x, y: y - x)(10)
print(isinstance(_k, abc.Hashable), isinstance(_k, abc.Callable))
