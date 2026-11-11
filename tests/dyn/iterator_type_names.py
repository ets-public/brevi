import itertools, collections, sys
vals = [[1], (1,), 'ab', 'é', range(3), {1: 2}, {1: 2}.keys(), {1: 2}.values(), {1: 2}.items(), {1}, frozenset([1]), b'ab', collections.deque([1])]
for v in vals:
    t = type(iter(v)); print(t.__name__, t.__module__ if t.__module__ != 'builtins' else '', t is type(iter(v)))
for it in [reversed([1]), reversed((1,)), reversed({1: 2}), reversed(range(2)), map(str, [1]), filter(None, [1]), zip([1]), enumerate([1]), (x for x in [1]), iter(int, 1),
           itertools.count(), itertools.chain([1]), itertools.islice([1], 1), itertools.repeat(1), itertools.cycle([1]), itertools.accumulate([1]), itertools.starmap(pow, [(1, 2)]),
           itertools.product([1]), itertools.permutations([1]), itertools.combinations([1], 1), itertools.groupby([1]), itertools.pairwise([1, 2]), itertools.compress([1], [1]),
           itertools.zip_longest([1]), itertools.takewhile(bool, [1]), itertools.dropwhile(bool, [1]), itertools.filterfalse(bool, [1])]:
    print(type(it).__name__, isinstance(it, list), isinstance(it, object))
class S(str): pass
class L(list): pass
print(type(iter(S('a'))).__name__, type(iter(L([1]))).__name__)
class G:
    def __getitem__(s, i):
        if i < 2: return i
        raise IndexError
print(type(iter(G())).__name__)
class It:
    def __iter__(s): return s
    def __next__(s): raise StopIteration
print(type(iter(It())).__name__, type(iter(It())) is It)
def gen(): yield 1
print(type(gen()).__name__, type(gen()) is type(x for x in []))
print(type(iter([])) is type(iter([1, 2])), type(iter(())) is type(iter([])))
e = iter([1]); next(e)
print(type(e).__name__)
print(isinstance(iter([]), type(iter([1]))), isinstance(iter(()), type(iter([1]))))
print(type(collections.Counter('ab').elements()).__name__)
import csv, io
r = csv.reader(io.StringIO('a,b\n'))
print(isinstance(r, list), isinstance(r, object))
