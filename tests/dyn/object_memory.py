# Instances share attribute layouts; nested defs are called directly; captured variables are read in place.
import copy


class P:
    def __init__(self, x, y):
        self.x = x
        self.y = y


a = P(1, 2)
b = P(3, 4)
b.z = 5  # b leaves a's layout
print(vars(a), vars(b))
c = P(6, 7)
c.w = 8
c.z = 9  # same names as b, another order
print(vars(c), list(c.__dict__))
del b.x
print(vars(b), hasattr(b, "x"))
b.x = 10
print(vars(b))


class Many:
    pass


m = Many()
for i in range(80):
    setattr(m, "a%d" % i, i)
print(len(vars(m)), m.a0, m.a79, list(vars(m))[:3], list(vars(m))[-2:])
for i in range(0, 80, 2):
    delattr(m, "a%d" % i)
print(len(vars(m)), m.a1, m.a79)
m2 = copy.copy(m)
m3 = copy.deepcopy(m)
m2.a1 = "copy"
print(m.a1, m2.a1, m3.a79, vars(m3) == vars(m))


class Slots:
    __slots__ = ("val", "grad", "kids", "dk")

    def __init__(self, v, kids=()):
        self.val = v
        self.grad = 0
        self.kids = kids
        self.dk = tuple(1 for _ in kids)


class Sub(Slots):
    def __init__(self, v):
        super().__init__(v)
        self.grad = 1.5


s = Sub(2.0)
print(s.val, s.grad, s.kids, s.dk)


class L(list):
    def __init__(self, *a):
        super().__init__(*a)
        self.tag = "t"


l = L([1, 2])
l.append(3)
print(l, l.tag, len(l))


# a nested def that calls itself, as an autograd's topological sort does
def topo(root):
    order = []
    seen = set()

    def visit(n):
        if n not in seen:
            seen.add(n)
            for k in n.kids:
                visit(k)
            order.append(n)

    visit(root)
    return order


leaf1, leaf2 = Slots(1.0), Slots(2.0)
mid = Slots(3.0, (leaf1, leaf2))
top = Slots(4.0, (mid, leaf1))
print([n.val for n in topo(top)])


# called before its def ran: NameError, as in CPython
def early():
    def g():
        return h()

    try:
        g()
    except NameError as e:
        print("NameError:", e)

    def h():
        return "h"

    return g()


print(early())


# rebound with nonlocal: not called directly
def rebound():
    def f():
        return 1

    def swap():
        nonlocal f
        old = f
        f = lambda: old() + 1

    a = f()
    swap()
    return a, f()


print(rebound())


# defined in a loop: each def captures the same cells
def in_loop():
    out = []
    total = 0
    for i in range(3):
        def add(n):
            return n + total + i

        total += 10
        out.append(add(i))
    return out


print(in_loop())


# a default argument and keyword calls still work through the function value
def defaults():
    k = 5

    def f(a, b=2):
        return a * b + k

    return f(1), f(1, 3), f(b=4, a=1), f(*[2])


print(defaults())


# captured values read in place while the owner changes them between calls
def counter():
    count = 0
    items = []

    def show(x):
        items.append((x, count))
        return len(items)

    for i in range(3):
        count += 1
        show(i)
    count = "done"
    show(9)
    return items


print(counter())


# recursion through a nested def with an error inside
def walk(n):
    def go(k):
        if k == 0:
            raise ValueError("bottom")
        return go(k - 1)

    try:
        go(n)
    except ValueError as e:
        return str(e)


print(walk(50))


# many objects made and dropped in rounds, as a training loop does
class Node:
    __slots__ = ("v", "kids")

    def __init__(self, v, kids=()):
        self.v = v
        self.kids = kids


total = 0.0
keep = None
for rnd in range(5):
    nodes = [Node(float(i)) for i in range(2000)]
    for i in range(2000, 6000):
        nodes.append(Node(nodes[i - 1].v * 0.5 + 1.0, (nodes[i - 1], nodes[i % 2000])))
    total += nodes[-1].v
    keep, prev = nodes, keep
print(round(total, 6), len(keep), keep[-1].kids[1].v)
