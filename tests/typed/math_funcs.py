# math functions and constants, from-imports of them, and time.perf_counter on the typed build.
import math
from math import gcd, pi, sqrt, log
import time
t0 = time.perf_counter()
print(gcd(12, 18), math.lcm(4, 6), math.comb(5, 2), math.perm(5, 2), math.factorial(25), math.comb(60, 30))
print(math.isclose(0.1 + 0.2, 0.3), math.isclose(1.0, 1.1, rel_tol=0.2))
print(sqrt(2), pi, math.e, math.tau, math.inf, -math.inf, log(8, 2), log(10), math.log2(8), math.log10(1000))
print(math.exp(1), math.sin(1.0), math.cos(0), math.atan2(1, 1), math.hypot(3, 4), math.fabs(-2), math.trunc(-2.7))
print(math.degrees(math.pi), math.radians(180), math.copysign(3, -1), math.fmod(7, 3), math.isnan(math.nan), math.isinf(math.inf), math.isfinite(1.0))
print(time.perf_counter() - t0 >= 0, time.time() > 1e9)
def area(r):
    return pi * r * r
print(round(area(2), 4))
