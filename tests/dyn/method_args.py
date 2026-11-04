import sys
xs = [1, 2, 3]
d = {"a": 1}
s = {1}
for f in (lambda: xs.clear(None), lambda: xs.copy(1), lambda: xs.reverse(42), lambda: d.items(None),
          lambda: d.keys(1), lambda: d.copy(None), lambda: s.copy(1)):
    try:
        f()
    except TypeError as e:
        print("TypeError", e)
d = {0: 0}
try:
    for k in d:
        del d[0]
        d[0] = 0
except RuntimeError as e:
    print("RuntimeError", e)
a = list(range(10))
print(a[::1 << 70], a.index(5, -4 * sys.maxsize, 4 * sys.maxsize))
try:
    int("0", base=2 ** 100)
except ValueError as e:
    print("ValueError", e)
try:
    {1: 2}.popitem(42)
except TypeError as e:
    print("popitem:", e)
from collections import OrderedDict
print(OrderedDict(a=1, b=2).popitem(last=False))
