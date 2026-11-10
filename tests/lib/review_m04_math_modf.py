import math
print(math.modf(-0.0))
print(math.modf(-3.5), math.modf(float("inf")))
for x in (0.0, -0.0, -3.5, 3.5, float("inf"), float("-inf"), float("nan"), -2.0, 2.0, -1e300, -0.5):
    print(math.modf(x))
