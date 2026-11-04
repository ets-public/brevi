# Library values in a dynamic build: calls, attributes, items, operators, iteration, formatting.
import brlib
from brlib import Store, Money, describe, apply, fill, countdown, total


def score(x, bonus=0):
    return x * 2 + bonus


s = Store()
s.add(3).add(1).add(2)
print(len(s), 2 in s, 5 in s, s[0], list(s), sorted(s))
print(s.items, s.meta, type(s.items).__name__, isinstance(s.items, list), isinstance(s.meta, dict))
s.items.append(9)
s.meta["extra"] = "x"
print(s.items, s.meta, len(s))
print([x + 1 for x in s], sum(s), max(s), ",".join(str(x) for x in s))
print(apply(score, [1, 2, 3], bonus=10), apply(lambda v: v - 1, (5, 6)))
xs, d = [1], {"a": 1}
fill(xs, d)
print(xs, d)
print(total(countdown(3)), total(x * x for x in range(4)))
for n in countdown(2):
    print("got", n)
m = Money(150) + Money(25)
print(m, m + 1.5, 1.5 + m, Money(1) < Money(2), Money(3) == Money(3), f"{m:.2f}", bool(Money(0)))
print({Money(1): "one"}[Money(1)], sorted([Money(5), Money(2)]))
print(describe([1, "a"]), describe({"k": (1, 2.5)}), describe(None), describe(2 ** 80), describe(b"hi"))
print(brlib.Model.__name__, type(s).__name__, repr(s.get(0)))
info = {"store": len(s), "first": s.get(1)}
print(info)
