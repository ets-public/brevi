# The descriptor protocol for the program's own classes: __get__ (data and non-data), __set__, __delete__ and
# __set_name__, with a data descriptor winning over the instance's attribute and a non-data one losing to it.
class D:
    def __get__(self, inst, owner=None):
        print("get", type(inst).__name__, owner.__name__)
        return self if inst is None else inst.__dict__.get("_x", 0) * 10
    def __set__(self, inst, v):
        print("set", v)
        inst.__dict__["_x"] = v
    def __delete__(self, inst):
        print("del"); inst.__dict__.pop("_x", None)
    def __set_name__(self, owner, name):
        print("set_name", owner.__name__, name)
class ND:
    def __get__(self, inst, owner=None): return "nd"
class C:
    x = D()
    y = ND()
    def __init__(self): self.z = 1
c = C()
print(c.x); c.x = 4; print(c.x); print(type(C.x).__name__)
for i in range(3): print(c.x, c.y, c.z)
c.y = "own"; print(c.y)
c.__dict__["x"] = 99; print(c.x)
del c.x; print(c.x)
class W:
    def __set__(self, i, v): print("only set")
class E:
    w = W()
e = E(); e.w = 1; print(type(e.w).__name__)
try: del e.w
except AttributeError as ex: print("AE", ex)

_NOT_FOUND = object()
class cached_property:
    def __init__(self, func):
        self.func = func
        self.attrname = None
        self.__doc__ = func.__doc__
        self.__module__ = func.__module__
    def __set_name__(self, owner, name):
        if self.attrname is None:
            self.attrname = name
        elif name != self.attrname:
            raise TypeError("Cannot assign the same cached_property to two different names "f"({self.attrname!r} and {name!r}).")
    def __get__(self, instance, owner=None):
        if instance is None:
            return self
        if self.attrname is None:
            raise TypeError("Cannot use cached_property instance without calling __set_name__ on it.")
        try:
            cache = instance.__dict__
        except AttributeError:
            msg = (f"No '__dict__' attribute on {type(instance).__name__!r} instance to cache {self.attrname!r} property.")
            raise TypeError(msg) from None
        val = cache.get(self.attrname, _NOT_FOUND)
        if val is _NOT_FOUND:
            val = self.func(instance)
            try:
                cache[self.attrname] = val
            except TypeError:
                msg = (f"The '__dict__' attribute on {type(instance).__name__!r} instance does not support item assignment for caching {self.attrname!r} property.")
                raise TypeError(msg) from None
        return val
class CC:
    n = 0
    @cached_property
    def cost(self):
        "doc"
        CC.n += 1
        return CC.n
class S:
    __slots__ = ('a',)
    @cached_property
    def cost(self): return 1
c = CC(); print(c.cost, c.cost, CC.cost.__doc__, CC.cost.__module__, isinstance(CC.cost, cached_property), c.__dict__)
try: S().cost
except TypeError as e: print(e)
try:
    class R:
        @cached_property
        def a(self): pass
        b = a
except TypeError as e: print(e)
class ro(cached_property):
    def __set__(self, obj, v): raise AttributeError("ro")
class T:
    def __init__(s,p): s._p=p
    @ro
    def prop(s): return s._p
t=T(1); print(t.prop); t._p=9; print(t.prop)


# obj.name(args) goes through the descriptor too (a call of what __get__ gives)
class MDesc:
    def __get__(self, inst, owner=None):
        return lambda *a: ("got", inst is None, a)


class MData(MDesc):
    def __set__(self, inst, v):
        raise AttributeError("read only")


class M:
    m = MDesc()
    d = MData()


mo = M()
print(mo.m(1), M.m(2), mo.d(3))
mo.__dict__["m"] = lambda *a: ("own", a)
mo.__dict__["d"] = lambda *a: ("own", a)
print(mo.m(4), mo.d(5))
