# A function returning `x.kids`, and its result changed (`kids_of(n).append(2)`, `q = kids_of(n)`):
# Python changes n.kids; the typed build changed a copy. Found by the copy-versus-alias sweep (9 Nov 2026).
class Node:
    def __init__(self):
        self.kids = []
n = Node()
k = n.kids
k.append(1)
def kids_of(x):
    return x.kids
kids_of(n).append(2)
q = kids_of(n)
q.append(3)
print(n.kids)
