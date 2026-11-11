# int() of an object whose __int__ or __index__ returns a strict subclass of int (bool here) gives a plain int
# with CPython 3.13's DeprecationWarning at the program's line; int() still delegates to __trunc__, with its
# own DeprecationWarning, and a __trunc__ result that is no integer is a TypeError.
import warnings

class I:
    def __int__(self):
        return True

class X:
    def __index__(self):
        return True

class T:
    def __trunc__(self):
        return X()

class Bad:
    def __trunc__(self):
        return "no"

with warnings.catch_warnings(record=True) as ws:
    warnings.simplefilter("always")
    for o in (I(), X(), T()):
        n = int(o)
        print(repr(n), type(n) is int)
    try:
        int(Bad())
    except TypeError as e:
        print("TypeError", e)
for w in ws:
    print(w.category.__name__, w.lineno, str(w.message)[:40])
with warnings.catch_warnings():
    warnings.simplefilter("error")
    try:
        int(I())
    except DeprecationWarning as e:
        print("raised", str(e)[:30])
