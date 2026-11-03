# Sets stored on objects and deques used as queues: kept as hash sets / VecDeques in typed builds.
from collections import deque


class Crawler:
    def __init__(self, n):
        self.n = n
        self.seen = set()
        self.order = []

    def links(self, x):
        return [(x * 2) % self.n, (x * 3 + 1) % self.n, (x + 11) % self.n]

    def visit(self, start):
        q = deque([start])
        self.seen.add(start)
        while q:
            x = q.popleft()
            self.order.append(x)
            for y in self.links(x):
                if y not in self.seen:
                    self.seen.add(y)
                    q.append(y)
        return len(self.seen)


class Inventory:
    def __init__(self):
        self.skus = set()

    def add(self, s):
        self.skus.add(s)

    def has(self, s):
        return s in self.skus


def main():
    c = Crawler(30000)
    print(c.visit(1), len(c.order), c.order[:5], c.order[-1], 5 in c.seen)
    inv = Inventory()
    for i in range(5000):
        inv.add("sku" + str(i * 7 % 3000))
    print(inv.has("sku42"), inv.has("sku3001"), len(inv.skus), sorted(inv.skus)[:3])
    d = deque()
    d.append(2)
    d.appendleft(1)
    d.extend([3, 4])
    d.insert(2, 9)
    print(len(d), d[0], d[-1], d[2], 9 in d, d.pop(), d.popleft(), len(d))
    total = 0
    while d:
        total += d.popleft()
    print(total, len(d))


main()
