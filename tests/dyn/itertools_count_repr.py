from itertools import count
print(repr(count(3)), repr(count(2.5, 1)), repr(count(1, 1.0)), repr(count(1j)), repr(count(start=2, step=3)))
c = count(1j); next(c); print(repr(c), list(zip("ab", count(2))))
for args in [("a",), (1, "b"), (1, 2, 3)]:
    try:
        count(*args)
    except TypeError as e:
        print(e)
