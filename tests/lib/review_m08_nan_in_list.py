nan = float("nan")
print(nan in [nan], [nan] == [nan])
print(nan in (nan,), (nan,) == (nan,))
print(float("nan") in [float("nan")], [float("nan")] == [float("nan")])
print(nan == nan, nan != nan, nan is nan)
import math
print(math.nan in [math.nan], [math.nan] == [math.nan], math.nan is math.nan)
from collections import deque
q = deque([nan])
print(nan in q, q == deque([nan]))
d = {nan: 7}
print(nan in d, d[nan], {1: nan} == {1: nan})
print(nan in {nan}, {nan} == {nan})
print([float("nan")] == [float("nan")], float("nan") is float("nan"))
print(float(nan) is nan, +nan is nan, -nan is nan)
# a NaN a computation makes is a new object, not the one it was computed from
b = nan + 1
print(b in [nan], [b] == [nan], b is nan)
print(nan * 2.0 is nan, abs(nan) is nan, nan ** 2 is nan, (nan - nan) is nan)
print(math.fabs(nan) is nan, math.sqrt(nan) is nan, round(nan, 2) is nan)
h = nan
h += 1.0
print(h is nan, h in [nan])
print(sum([nan, 1]) is nan)
print(max(nan, 1) is nan, max(nan, 1) in [nan])
# a NaN the floating-point hardware made is still one object
a = float("inf") - float("inf")
print(a in [a], [a] == [a], (a,) == (a,))
print(a in {a}, {a} == {a}, a in {a: 1}, {a: 1}[a])
# dict value views take an identical member before comparing it
print(nan in {1: nan}.values())
