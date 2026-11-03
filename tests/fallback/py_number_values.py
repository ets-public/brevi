# Values that start with a number inside f-strings, a negative literal power, and raise SystemExit(n).
import sys

n = 3
print(f"{1/3:.3f} {2*n} {n/2:.2f} {.5:.1f} {3}")
print(f'{1/4:.2f} {2*n}')
print("a{1}b", "{}".format(n), 10 ** -2, 2 ** -1, n ** 2)
print("done")
raise SystemExit(3)
