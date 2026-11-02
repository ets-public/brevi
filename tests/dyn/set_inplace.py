# In-place set operators and methods: CPython's results and order, in time with the other set's size.
a = {5, 1, 9, -3, 12, 40}
a |= {7, 1, 100}
print(a, len(a))
a -= {9, 40, 999}
print(a)
a &= {1, 5, 7, 100, 12, 8}
print(a)
a ^= {1, 2}
print(a)
b = set()
for i in range(50000):
    b.add((i % 250, i // 250))
seen = set()
for i in range(50000):
    seen |= {i % 997}
print(len(b), (3, 4) in b, len(seen), b.issuperset({(0, 0), (1, 0)}), {(0, 0)}.issubset(b))
b.update([(-1, -1)])
b.difference_update({(0, 0)})
print(len(b), (-1, -1) in b, (0, 0) in b)
