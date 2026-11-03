# Functions of Brevi's own modules that the typed build lacks run natively (the dynamic build), not in CPython.
import math
import string
import itertools

print(math.sqrt(16), math.gcd(12, 18), math.floor(2.5), math.isclose(0.1 + 0.2, 0.3), math.comb(5, 2))
print(string.ascii_lowercase[:3], string.digits)
print(list(itertools.permutations([1, 2, 3], 2)))
print(list(itertools.product("ab", repeat=2)))
