# The typed build would hold a reference to bx.rows[0] while f grows bx.rows (use after free): dynamic build.
class Box:
    def __init__(self):
        self.rows = [[1]]

def f(b: Box) -> int:
    for i in range(1000):
        b.rows.append([i, i, i, i])
    return 5

bx = Box()
bx.rows[0].append(f(bx))
print(bx.rows[0], len(bx.rows))
