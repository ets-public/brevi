# f rebinds bx.items while the typed build holds the old field for append: dynamic build.
class Box:
    def __init__(self):
        self.items = [1, 2, 3]

def f(b: Box) -> int:
    b.items = [9]
    return 5

def k(b: Box) -> int:
    b.items.append(4)
    return 0

bx = Box()
bx.items.append(f(bx))
print(bx.items)
by = Box()
by.items.insert(k(by), 7)
print(by.items)
bz = Box()
print(bz.items[k(bz)], bz.items)
