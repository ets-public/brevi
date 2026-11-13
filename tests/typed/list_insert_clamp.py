# list.insert clamps an index past either end, as CPython does
a = [1, 2, 3]
for i in (-100, 100, -1, 3, 0, -3, 7):
    a.insert(i, i)
print(a)
