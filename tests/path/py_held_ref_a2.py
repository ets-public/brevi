# bump(c.items, c) changes the list through both names: dynamic build.
class C:
    def __init__(self):
        self.items = [1, 2, 3]
        self.n = 0

def bump(xs: list[int], c: C) -> int:
    xs.append(5)
    c.items.append(7)
    xs.append(9)
    return len(xs) + len(c.items)

def cnt(c: C, d: C) -> int:
    c.n = 1
    d.n = 2
    return c.n

c = C()
print(bump(c.items, c), c.items[-3:])
print(cnt(c, c))
