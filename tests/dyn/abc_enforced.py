# abstract methods are enforced on abc.ABC and metaclass=ABCMeta classes, including properties, classmethods and singledispatchmethod
import functools
import abc
from abc import ABC, abstractmethod
class P(ABC):
    @abstractmethod
    def f(self): ...
class M(metaclass=abc.ABCMeta):
    @abstractmethod
    def f(self): ...
for k in (P, M):
    try: k(); print('NO-RAISE')
    except TypeError as e: print('raises', e)
print(P.__abstractmethods__, M.__abstractmethods__)
class Abstract(metaclass=abc.ABCMeta):
    @functools.singledispatchmethod
    @abc.abstractmethod
    def add(self, x, y):
        pass
print(repr(bool(Abstract.add.__isabstractmethod__)))
print(repr(bool(Abstract.__dict__['add'].__isabstractmethod__)))
try:
    Abstract()
except TypeError as _e:
    print('raises', type(_e).__name__, _e)
else:
    print('NO-RAISE')
class P(abc.ABC):
    @property
    @abc.abstractmethod
    def x(self): ...
    @classmethod
    @abc.abstractmethod
    def c(cls): ...
    def y(self): ...
    y.__isabstractmethod__ = False
for k in (P,):
    try: k(); print('NO-RAISE')
    except TypeError as e: print('raises', e)
print(sorted(P.__abstractmethods__))
class Q(P):
    @property
    def x(self): return 1
    @classmethod
    def c(cls): return 2
print(Q().x, Q.c())
class P0:
    @property
    @abc.abstractmethod
    def x(self): ...
    @property
    def y(self): return 1
    @classmethod
    def c(cls): ...
print(P0.__dict__['x'].__isabstractmethod__, P0.__dict__['y'].__isabstractmethod__, P0.__dict__['c'].__isabstractmethod__)
