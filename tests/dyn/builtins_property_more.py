import builtins
P = builtins.property
class D:
    def __init__(self): self.v = 3
    @P
    def a(self): return self.v
    def _g(self): return self.v + 1
    def _d(self): print("deleting"); del self.v
    b = P(_g, None, _d)
    c = P(fget=_g, doc="cdoc")
    e = P(_g).setter(lambda self, x: setattr(self, "v", x))
o = D()
print(o.a, o.b, o.c, D.c.__doc__, o.e)
o.e = 10
print(o.a, o.e)
del o.b
print(hasattr(o, "v"))
print(builtins.property is property)
class Sub(D):
    b = D.b.getter(lambda self: "sub")
print(Sub().b)
