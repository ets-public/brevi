# vars(C), C.__dict__ and dir() as CPython 3.13 gives them (rt_tdict.rs); private names mangled (mangle.rs)
import collections.abc as cabc
from collections import UserDict, UserList, UserString, namedtuple


class A:
    """doc"""
    x = 1
    __y = 2

    def __init__(self):
        self.b = 1
        self.__c = 2

    def m(self):
        self.a = 3

        def inner():
            self.z = 4


class B(A):
    __slots__ = ('q', '__r')

    def __eq__(self, o):
        return True


class E(Exception):
    pass


class I(int):
    pass


class L:
    x = 1
    print(list(locals()))


print(list(vars(A)))
print(A.__firstlineno__, A.__static_attributes__, vars(A())['_A__c'], A._A__y)
print(list(vars(B)), repr(B.q), repr(B.__dict__['__doc__']))
print(list(vars(E)), list(vars(I)))
print(list(vars(cabc.MutableMapping)))
print(list(vars(UserDict))[-4:], vars(UserDict)['__firstlineno__'])
print(list(vars(namedtuple('P', 'x y'))))
print(list(vars(dict))[:6], len(vars(dict)), repr(vars(dict)['keys']), repr(vars(str)['__add__']))
print(set(dir(UserDict)) >= set(dir(dict)), set(dir(UserList)) >= set(dir(list)), set(dir(UserString)) >= set(dir(str)))
print(dir(A())[-4:], '__weakref__' in dir(namedtuple('P', 'x')(1)))
print(repr(cabc.Iterable[int]), repr(vars(cabc.Iterable)['__class_getitem__']))
try:
    vars(cabc.Callable)
except NotImplementedError as e:
    print("refused")  # Brevi: Callable has no _CallableGenericAlias, so its dict refuses (CPython prints nothing here)
