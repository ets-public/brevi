# A lambda whose call grows bx.items while the typed build holds the field for append (a use-after-free
# before the held_refs pass): the dynamic build.
class Box:
    def __init__(self):
        self.items = [1, 2, 3]

def bump(b: Box) -> int:
    b.items.extend([7, 8])
    return 5

grow = lambda b: bump(b)
bx = Box()
bx.items.append(grow(bx))
print(bx.items)
