# math domain errors are ValueErrors an except catches, as in CPython.
import math
from math import sqrt


def safe_log(x):
    try:
        return math.log(x)
    except ValueError as e:
        print("error:", e)
        return -1.0


print(safe_log(0.0), safe_log(1.0), safe_log(math.e))
for x in [4, -1]:
    try:
        print(sqrt(x), math.log(x, 2), math.acos(x / 4))
    except ValueError:
        print("bad", x)
