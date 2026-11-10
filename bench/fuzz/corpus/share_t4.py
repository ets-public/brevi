d = {}
for w in ["a", "b", "a"]:
    d.setdefault(w, []).append(len(w))
print(d)
def solve(n):
    order = []
    def visit(v):
        order.append(v)
        if v < n:
            visit(v + 1)
    visit(0)
    return order
print(solve(3))
class Bag:
    def __init__(self):
        self.xs = []
    def put(self, out, v):
        out.append(v)
b = Bag()
o = [1]
b.put(o, 2)
print(o)
