import itertools

def count_up(start):
    n = start
    while True:
        yield n
        n += 1

def take(it, k):
    out = []
    for x in it:
        if len(out) == k:
            break
        out.append(x)
    return out

def inner():
    x = yield 1
    print("inner got", x)
    yield 2
    return "done-inner"

def outer():
    r = yield from inner()
    print("outer saw", r)
    yield from [10, 20]
    yield from range(3)

def averager():
    total = 0
    count = 0
    avg = None
    while True:
        v = yield avg
        total += v
        count += 1
        avg = total / count

def walk(tree):
    if tree is None:
        return
    left, val, right = tree
    yield from walk(left)
    yield val
    yield from walk(right)

class Board:
    def __init__(self, n):
        self.n = n
    def cells(self):
        for r in range(self.n):
            for c in range(self.n):
                yield (r, c)
    def __iter__(self):
        yield from self.cells()

def failing():
    yield 1
    raise ValueError("boom")

def make_counter(step):
    def gen(k):
        for i in range(k):
            yield i * step
    return gen

def chatty():
    print("start")
    yield "a"
    print("middle")
    yield "b"
    print("end")

print(take(count_up(5), 4))
g = outer()
print(next(g))
print(g.send("hello"))
print(list(g))
a = averager()
next(a)
print(a.send(10), a.send(20), a.send(60))
t = ((None, 1, None), 2, ((None, 3, None), 4, None))
print(list(walk(t)))
print(len(list(Board(3))), list(Board(2).cells()))
try:
    for x in failing():
        print("got", x)
except ValueError as e:
    print("caught", e)
print(list(make_counter(3)(4)))
for x in chatty():
    print("loop", x)
gs = [count_up(i * 100) for i in range(3)]
print([next(g) for g in gs], [next(g) for g in gs])
print(sum(x for x in count_up(0) if x % 7 == 0 and x < 50) if False else sum(itertools.islice(count_up(1), 10)))
g2 = count_up(0)
print(list(zip(g2, "xyz")), next(g2))
def deep(n):
    if n == 0:
        yield 0
        return
    for x in deep(n - 1):
        yield x + 1
print(list(deep(200)))[-1:] if False else print(max(deep(200)))
def stopper():
    yield 1
    return 5
s = stopper()
print(next(s))
try:
    next(s)
except StopIteration as e:
    print("stop", e.value)
c = count_up(0); next(c); c.close()
print(list(c))
