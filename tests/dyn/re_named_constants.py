# re._constants.MAXREPEAT is a _NamedIntConstant: an int whose str and repr are its name; MAXGROUPS is a plain int.
# re._compiler imports both, so they are the same objects there.
import re
import re._constants
import re._compiler
from re._constants import MAXREPEAT, MAXGROUPS

m = re._compiler.MAXREPEAT
print(m, repr(m), str(m), f"{m}", "%d" % m, format(m, "x"), f"{m:>12}")
print(int(m), m + 0, m - 1, m >= 0, m == 4294967295, hash(m) == hash(4294967295))
print(type(m).__name__, type(m).__module__, isinstance(m, int), m.name)
print(m is MAXREPEAT, m is re._constants.MAXREPEAT, re._compiler.MAXGROUPS == MAXGROUPS)
g = re._compiler.MAXGROUPS
print(g, repr(g), type(g).__name__, g >= 0)
print([m], {m: 1}, (m,))
print(re._compiler.MAXREPEAT >= 0, re._compiler.MAXGROUPS >= 0)
k = type(m)(7, "SEVEN")
print(k, k + 1, k.name, type(k).__name__)
d = {4294967295: "x"}
print(d[m])
# the submodules as attributes read at run time
import json
def sub(mod, name):
    return getattr(mod, name)
print(sub(re, "_compiler").MAXREPEAT, sub(re, "_constants").MAXGROUPS, sub(json, "decoder").JSONDecodeError.__name__)
