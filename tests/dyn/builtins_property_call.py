import builtins
class A:
    @property
    def a(self):
        "a's doc"
        return 1
    def g(self):
        "g doc"
        return 2
    b = builtins.property(g)
    c = property(g, doc="given")
    d = builtins.property(lambda self: 3)
    e = property(g).setter(lambda self, v: None)
print(A.a.__doc__, A.b.__doc__, A.c.__doc__, A.d.__doc__, A.e.__doc__)
o = A()
print(o.a, o.b, o.c, o.d)
try:
    del o.a
except AttributeError as e:
    print(e)
try:
    o.a = 5
except AttributeError as e:
    print(e)
print(A.b.fget is A.g, A.b.fset, A.b.fdel)
print(builtins.len([1, 2]), builtins.sorted([3, 1]), builtins.isinstance(1, int), builtins.abs(-3))
try:
    property(1, 2, 3, 4, 5)
except TypeError as e:
    print("TypeError", e)
