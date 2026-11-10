# functools.cmp_to_key keys compare by calling the wrapped cmp function on their wrapped values, so two keys
# that wrap equal values are equal (and order by the cmp result), while comparing against a non-key raises.
from functools import cmp_to_key

k = cmp_to_key(lambda a, b: (a > b) - (a < b))
a, b, c = k(1), k(1), k(2)
print(a == b, a == c, a != b, a != c)
print(a < b, a <= b, a > c, a >= c, b < c)

print(sorted([3, 1, 2], key=k))
print(sorted(["bb", "a", "ccc"], key=cmp_to_key(lambda x, y: (len(x) > len(y)) - (len(x) < len(y)))))

for other in (None, 3, "x"):
    for f in (lambda: a == other, lambda: other == a, lambda: a != other, lambda: other != a,
              lambda: a < other, lambda: other < a, lambda: a <= other, lambda: other <= a,
              lambda: a > other, lambda: other > a, lambda: a >= other, lambda: other >= a):
        try:
            print(f())
        except TypeError as e:
            print("TypeError", e)

try:
    print(hash(a))
except TypeError as e:
    print("TypeError", e)

kf = cmp_to_key(lambda x, y: float(x) - float(y))
print(kf(1.5) == kf(1.5), kf(1.0) < kf(2.0))
kb = cmp_to_key(lambda x, y: x < y)
print(kb(1) == kb(2), kb(1) < kb(2))
