import collections
from collections import ChainMap, UserDict, UserList, UserString, deque, Counter
import collections.abc as A
c = ChainMap({'a': 1}, {'a': 2, 'b': 3})
print(c['a'], c['b'], len(c), sorted(c), c, repr(type(c)), type(c).__module__)
print(isinstance(c, A.MutableMapping), issubclass(UserList, A.MutableSequence), isinstance(UserString('x'), A.Sequence))
print(c.keys() & {'a', 'z'}, dict(c | {'z': 0}))
c2 = c.new_child({'q': 9}); print(c2.parents is not None, c2['q'], list(c2.items()))
u = UserDict(x=1); u['y'] = 2; print(u, u.get('x'), 'y' in u, u | {'k': 1}, type(u | {}).__name__)
l = UserList([3, 1, 2]); l.sort(); print(l, l[1:], type(l[1:]).__name__, l * 2, l + [9], 3 in l)
s = UserString('hello'); print(s.upper(), type(s.upper()).__name__, s[1:3], s + '!', s.count('l'), ''.join(UserList(['a','b'])))
print(collections.ChainMap is ChainMap, hasattr(collections, 'UserDict'), collections.deque([1]), Counter('aab').most_common(1))
d = deque([1,2]); print(isinstance(d, A.MutableSequence))
class M(UserDict):
    def __missing__(self, k): return k * 2
print(M()[4])
import copy; print(copy.copy(c), copy.deepcopy(l))
print(ChainMap({1:2}) == {1:2}, UserList([1]) == [1], UserString('a') < 'b')
print(sorted(collections.__all__))
s = UserString('Ab c')
print(str(s), s.split(), s.replace('A', 'x'), s * 2, 'b' in s, hash(s) == hash('Ab c'), s.isalpha())
l = UserList(range(5)); l.append(7); l.extend([8]); l.insert(0, -1); l.remove(3); print(l, l.pop(), l.index(4), l.count(1), reversed(l).__class__.__name__)
l[1:3] = [10, 11]; del l[0]; print(l, len(l), sorted(l, reverse=True))
cm = ChainMap({'a': 1}); cm['b'] = 2; del cm['a']
try:
    del cm['zz']
except KeyError as e:
    print('KeyError', e)
print(cm, cm.maps, cm.pop('b'), cm.get('q', 5), bool(cm), ChainMap(), ChainMap.fromkeys('ab', 0))
cm |= {'x': 1}; print(cm, {'y': 2} | cm)
def g():
    yield 1
print(g().__class__.__name__, (x for x in []).__class__ is type(g()))
