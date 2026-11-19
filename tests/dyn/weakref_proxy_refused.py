# weakref.proxy refuses what it cannot do exactly: a callback, and a referent with numeric conversions. A
# refusal stops the program: the `except` never sees it (check.sh checks the callback refusal the same way).
import weakref
class N:
    def __index__(self): return 1
print("start")
try:
    weakref.proxy(N())
except NotImplementedError as e:
    print("caught", e)
print("not reached")
