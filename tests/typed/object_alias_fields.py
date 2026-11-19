# Two names for one object: each statement below reaches the same object twice, once to change a field and
# once to read it (or to change it in a call). The typed build must not hold two `&mut` borrows of the object
# at once (tools/miri_emitted.py runs this file under Miri).


class Node:
    def __init__(self, v):
        self.v = v
        self.other = None
        self.xs = []
        self.ws = [3, 1, 2]

    def set_other_then_read(self):
        self.v = 1
        self.other.v = 2
        return self.v

    def grow(self):
        self.xs.append(len(self.xs))
        return len(self.xs)

    def sorted_by_weight(self):
        self.xs.sort(key=lambda i: self.ws[i % 3])
        return self.xs


def bump(n):
    n.v += 10
    n.xs.append(n.v)
    return n.v


n = Node(0)
n.other = n
print(n.set_other_then_read())
a = Node(3)
b = a
a.xs.append(b.v)
b.xs.append(len(a.xs))
print(a.xs, b.xs)
a.v = a.v + b.v
print(a.v)
a.xs.append(bump(b))
print(a.xs, a.v)
print(a.grow(), b.grow())
a.xs.insert(0, b.xs[len(b.xs) - 1])
print(a.xs)
print(a.xs.count(b.xs[0]), b.xs.index(a.xs[1]))
print(a.sorted_by_weight())
a.xs.extend(b.ws)
print(b.xs)
