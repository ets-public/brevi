from collections import namedtuple
from itertools import repeat

# itertools.repeat keeps its element and the calls left, so repr shows CPython's count
r = repeat('a', 3)
print(repr(r), next(r), repr(r))
print(list(r), repr(r))
print(repr(repeat(1, -5)), list(repeat(1, -5)))
print(repr(repeat([1, 2], times=2)), list(repeat([1, 2], times=2)))
inf = repeat(None)
print(repr(inf), [next(inf) for _ in range(3)], repr(inf))
print(list(zip(range(4), repeat(0))))
print(len(list(repeat(7, 100000))))

# each namedtuple field descriptor has its own __doc__
Point = namedtuple('Point', 'x y')
print(repr(Point.x.__doc__), repr(Point.y.__doc__), repr(Point.x))
Point.x.__doc__ = 'docstring for Point.x'
print(repr(Point.x.__doc__), repr(Point.x))
Vector = namedtuple('Vector', 'x y')
print(repr(Vector.x.__doc__))
del Point.y.__doc__
print(repr(Point.y.__doc__), repr(Point.y))
Point.y.__doc__ = None
print(repr(Point.y))
print(type(Point.x).__name__, Point(1, 2).x, Point(1, 2).y)
try:
    Point.x.foo
except AttributeError as e:
    print('AttributeError', e)
try:
    Point.x.foo = 1
except AttributeError as e:
    print('AttributeError', e)
print(type(Point.x).__module__, type(Point.x).__qualname__)
