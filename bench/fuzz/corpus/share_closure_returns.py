def outer():
    a = [1]
    def get():
        return a
    b = get()
    b.append(2)
    print(a)
outer()
xs = [[1], [2]]
def first():
    return xs[0]
first().append(5)
y = first()
y.append(6)
print(xs)
class C:
    def __init__(self):
        self.items = []
    def get(self):
        return self.items
c = C()
g = c.get
g().append(1)
print(c.items)
ops = {"a": lambda: 1}
print(ops["a"]())
