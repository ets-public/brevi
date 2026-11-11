# A builtin exception class CPython hands back through the bridge is the program's own class:
# `is`, issubclass and except clauses see the same class as CPython.
import warnings
warnings.simplefilter("error", FutureWarning)
c = warnings.filters[0][2]
print(c is FutureWarning, issubclass(c, Warning), c.__name__, c == FutureWarning)
try:
    raise c("raised")
except FutureWarning as e:
    print("caught", type(e).__name__, e)
