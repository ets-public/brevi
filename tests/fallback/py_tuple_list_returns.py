# A function returning a real tuple on one path and, on another, a one-item tuple joined with a name that
# holds tuple(...): the typed build keeps that one as a list, which would print as [5, ...] and sort below
# every tuple, so the file takes the dynamic build and gives CPython's order and output.
def score(kind, ranks):
    if kind == "straight":
        return (4, ranks[0])
    rest = tuple(ranks)
    return (5,) + rest


hands = [("ann", score("straight", [9, 8])), ("dan", score("flush", [8, 6, 5]))]
hands.sort(key=lambda h: h[1], reverse=True)
for name, s in hands:
    print(name, s)
