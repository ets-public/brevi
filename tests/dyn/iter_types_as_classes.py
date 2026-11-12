import itertools, operator
# Iterator types are classes: type(x) is the name, isinstance works, subclasses run.
print(type(itertools.chain([1])) is itertools.chain, type(zip([1])) is zip, type(map(str, [1])) is map)
print(isinstance(filter(None, [1]), filter), isinstance(enumerate([1]), enumerate), isinstance(reversed([1]), reversed))
print(type(itertools.count()) is itertools.count, type(itertools.chain.from_iterable([[1]])) is itertools.chain)
print(itertools.chain.__name__, zip.__name__, map.__name__, itertools.count.__module__)
class C(itertools.chain):
    pass
c = C([1, 2], [3])
print(type(c).__name__, list(c), isinstance(c, itertools.chain))
print(type(C.from_iterable([[4], [5]])).__name__, list(C.from_iterable([[4], [5]])))
class Z(zip):
    def __next__(self):
        return sum(super().__next__())
print(list(Z([1, 2], [10, 20])))
class M(map):
    pass
print(list(M(str, [1, 2])), type(M(str, [])).__name__)
class E(enumerate):
    pass
print(list(E('ab', 1)))
class F(filter):
    pass
print(list(F(None, [0, 1, 2])))
class Cnt(itertools.count):
    pass
k = Cnt(5)
print(next(k), next(k), repr(k))
class R(itertools.repeat):
    pass
print(list(R('x', 2)), repr(R('x', 2)))
class A(itertools.accumulate):
    pass
print(list(A([1, 2, 3])))
class I(itertools.islice):
    pass
print(list(I(range(10), 2, 6, 2)))
for k in [itertools.chain, itertools.count, itertools.islice, itertools.starmap, itertools.cycle, zip, map, filter]:
    try:
        k(x=1)
    except TypeError as e:
        print(k.__name__, 'TypeError')
for base in [type(iter([])), type(iter(())), type(x for x in [])]:
    try:
        class X(base): pass
        print('subclassed', base.__name__)
    except TypeError as e:
        print('refused', e)
try:
    zip([1], bogus=True)
except TypeError as e:
    print(e)
print(list(zip([1, 2], [3], strict=False)))
a = [1]
print(operator.is_(a, a), operator.is_(a, [1]), operator.is_not(a, [1]), operator.is_not(None, None))
