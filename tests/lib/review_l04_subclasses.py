class A: pass
class B(A): pass
print([c.__name__ for c in A.__subclasses__()], B.__mro__[1].__name__, B.__bases__)
class C(A): pass
class D(C): pass
print([c.__name__ for c in A.__subclasses__()])
print([c.__name__ for c in C.__subclasses__()])
print([c.__name__ for c in B.__subclasses__()])
print(type.__subclasses__(A))
print(A.__bases__, C.__bases__)
def make():
    class E(A): pass
    return A.__subclasses__()
print([c.__name__ for c in make()])
try:
    A.__subclasses__(1)
except TypeError as e:
    print(e)
