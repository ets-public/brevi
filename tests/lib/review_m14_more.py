from functools import cmp_to_key
k = cmp_to_key(lambda a, b: (a > b) - (a < b))
print(k(1) == k(1), k(1) != k(1), k(1) != k(2), k(1) in [k(0), k(1)], [k(1)] == [k(1)], (k(2),) == (k(2),))
print(sorted([3, 1, 2], key=k), max([3, 1, 2], key=k), min([k(3), k(1)]).obj)
for c in (lambda: k(1) == 1, lambda: 1 == k(1), lambda: k(1) == None, lambda: None == k(1), lambda: k(1) < 1, lambda: {k(1): 1}):
    try: print(c())
    except TypeError as e: print("TE", e)
calls = []
def cmp(a, b): calls.append((a, b)); return a - b
kk = cmp_to_key(cmp); print(kk(5) == kk(5), calls)
def bad(a, b): raise ValueError("boom")
try: print(cmp_to_key(bad)(1) == cmp_to_key(bad)(2))
except ValueError as e: print("VE", e)
