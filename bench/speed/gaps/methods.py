# Method calls, attribute reads and small objects: the generic dynamic-build cost behind
# the jinja2, pydantic and polars gaps. Prints a checksum; time goes to stderr.
import sys
import time


class Vec:
    def __init__(self, x, y):
        self.x = x
        self.y = y

    def add(self, o):
        return Vec(self.x + o.x, self.y + o.y)

    def dot(self, o):
        return self.x * o.x + self.y * o.y

    def scale(self, k):
        self.x *= k
        self.y *= k
        return self


class Counter:
    def __init__(self):
        self.n = 0
        self.items = []

    def bump(self, v):
        self.n += 1
        if v % 3 == 0:
            self.items.append(v)

    def get(self):
        return self.n


def run(n):
    a = Vec(1, 2)
    b = Vec(3, 4)
    c = Counter()
    s = 0
    for i in range(n):
        v = a.add(b)
        s += v.dot(a)
        c.bump(i)
        if i % 1000 == 0:
            a = Vec(i % 7, i % 5)
        s += c.get()
    return s + len(c.items)


t = time.perf_counter()
print(run(int(sys.argv[1]) if len(sys.argv) > 1 else 1000000))
print("time %.3f" % (time.perf_counter() - t), file=sys.stderr)
