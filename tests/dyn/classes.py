import sys
from collections import Counter, defaultdict

class Shape:
    count = 0
    def __init__(self, name):
        self.name = name
        Shape.count += 1
    def area(self):
        return 0
    def __repr__(self):
        return f"{type(self).__name__}({self.name!r}, area={self.area():.2f})"

class Sq(Shape):
    def __init__(self, s):
        super().__init__("sq")
        self.s = s
    def area(self):
        return self.s ** 2

def fib(n, memo={}):
    if n < 2:
        return n
    if n in memo:
        return memo[n]
    r = fib(n - 1) + fib(n - 2)
    memo[n] = r
    return r

def make_counter():
    c = 0
    def inc(k=1):
        nonlocal c
        c += k
        return c
    return inc

rows = [("a", 1, 2.5), ("b", [1, 2], None), ("c", {"x": 1}, True)]
for name, v, w in rows:
    print(name, v, w)
inc = make_counter()
inc(); inc(5)
print(inc(), fib(90), Shape.count)
print([Sq(3), Shape("blob")], Shape.count)
d = defaultdict(list)
for i, ch in enumerate("hello world"):
    d[ch].append(i)
print(dict(d))
print(Counter("mississippi").most_common(2))
try:
    x = {}["k"]
except KeyError as e:
    print("missing", e)
finally:
    print("done")
vals = sorted({3, 1, 2}, key=lambda x: -x)
print(vals, sum(x * x for x in range(10)), 2 ** 100, 7 // -2, -7 % 3, 1 / 3)
for i in range(3):
    if i == 5:
        break
else:
    print("no break")
s = "a,b,,c"
print(s.split(","), "-".join(reversed(s.split(","))), f"{3.14159:>10.3f}|{42:05d}|{'x':^5}|")
