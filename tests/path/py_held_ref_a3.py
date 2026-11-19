# A list passed beside the object that holds it, changed through the object: dynamic build.
class Box:
    def __init__(self):
        self.items = [1, 2, 3]
        self.n = 0

def f(xs: list[int], b: Box) -> int:
    a = len(xs)
    b.items.append(1)
    return len(xs) - a

def g(xs: list[int], b: Box) -> int:
    t = 0
    for i in range(1000):
        t += xs[0]
        b.items[0] = i
    return t

def h(xs: list[int], b: Box) -> None:
    for i in range(100):
        xs.append(i)
        b.items.append(-i)
    print(len(xs), sum(xs))

bx = Box()
print(f(bx.items, bx))
print(g(bx.items, bx))
h(bx.items, bx)
