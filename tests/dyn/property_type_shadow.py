# property is a type, a data descriptor that wins over an instance attribute, and checks its arguments as CPython does
p = property(lambda s: 1)
print(isinstance(p, property), isinstance(1, property), isinstance(p, (int, property)))
print(type(p) is property, type(p).__name__, type(p), repr(property))
print(type(p)(lambda s: 9).fget(None))
for args, kw in [((1,), {"fget": 2}), ((), {"bad": 1}), ((1, 2), {"doc": 3, "fdel": 4, "fset": 5}), ((), {"fdel": 1, "fget": 2})]:
    try:
        q = property(*args, **kw)
        print("ok", q.fget, q.fdel)
    except TypeError as e:
        print("TypeError", e)
class C:
    def __init__(self): self.v = 1
    @property
    def b(self): return "prop"
    @property
    def m(self): return lambda: "called"
o = C()
o.__dict__["b"] = 1
o.__dict__["m"] = lambda: "dict"
print(o.b, getattr(o, "b"), o.m(), o.__dict__["b"])
q = C()
q.z = 5
print(q.z)
C.z = property(lambda self: "late")
print(q.z, q.v)
class N:
    x = property(None, lambda s, v: None)
try:
    N().x
except AttributeError as e:
    print(e)
