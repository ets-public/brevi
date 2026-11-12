from itertools import count, repeat, islice
print(list(islice(count(3.25-4j), 3)), list(islice(count(2, 3.25-4j), 3)), repr(count(1j)), repr(count(1, 2j)), repr(repeat(1+0j)), repr(repeat(1+0j, 5)), list(repeat(1j, 2)))
c = count(1j); next(c); print(repr(c))
