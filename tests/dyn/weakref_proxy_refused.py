# weakref.proxy refuses what it cannot do exactly: a callback, and a referent with numeric conversions
import weakref
class N:
    def __index__(self): return 1
class A:
    pass
a = A()
for f in [lambda: weakref.proxy(N()), lambda: weakref.proxy(a, print)]:
    try:
        f()
    except NotImplementedError as e:
        print(e)
