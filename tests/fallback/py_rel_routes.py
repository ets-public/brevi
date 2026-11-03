# Programs the typed build would get wrong, which brevi run builds over dynamic values instead.
from fractions import Fraction


# A list slot set by a nested function, where the list holds an int and None (sudoku solution counter).
def count(grid, limit=2):
    grid = list(grid)
    found = [0, None]

    def search(g):
        if found[0] >= limit:
            return
        best = None
        for i in range(len(g)):
            if g[i] == 0:
                best = i
                break
        if best is None:
            found[0] += 1
            if found[0] == 1:
                found[1] = list(g)
            return
        for d in [1, 2]:
            g[best] = d
            search(g)
            g[best] = 0

    search(grid)
    return found[0], found[1]


print(count([0, 1, 0]))
print(count([1, 1]))

# Fractions with any denominator (a unit table).
day = Fraction(1, 86400)
inch = Fraction(127, 5000)
print(day * 3600, inch / Fraction(1, 100), Fraction(762, 25) / 100)
print(1 / (Fraction(1, 3600) * Fraction(1, 24)))
