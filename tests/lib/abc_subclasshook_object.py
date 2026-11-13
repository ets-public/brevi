# issubclass on an ABCMeta class with no __subclasshook__ of its own uses object's (a class method bound to the class)
from abc import ABCMeta


class B(metaclass=ABCMeta):
    pass


class C(B):
    pass


class V:
    pass


print(issubclass(V, B), issubclass(C, B))
B.register(V)
print(issubclass(V, B), isinstance(V(), B))
print(B.__subclasshook__(V), C.__subclasshook__(int))
print(object.__subclasshook__(V))
