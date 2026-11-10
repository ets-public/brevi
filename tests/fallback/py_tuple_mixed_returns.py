# A function returning a real tuple on one path and `(5,) + tuple(xs)` on another (a poker hand score):
# the typed build keeps the second as a list, which printed as [5, ...] and sorted below every tuple.
# Found by the DeepSeek release test (poker, 9 Nov 2026). It must give CPython's order and output.
def score(kind, ranks):
    if kind == "straight":
        return (4, ranks[0])
    if kind == "flush":
        return (5,) + tuple(ranks)
    if kind == "two pair":
        return (2, ranks[0], ranks[1], ranks[2])
    return (0,) + tuple(ranks)


hands = [
    ("ann", score("straight", [9, 8, 7, 6, 5])),
    ("cat", score("two pair", [9, 4, 14])),
    ("dan", score("flush", [8, 6, 5, 4, 3])),
    ("vic", score("high", [13, 11, 9, 7, 2])),
]
hands.sort(key=lambda h: h[1], reverse=True)
for name, s in hands:
    print(name, s)
print(max(h[1] for h in hands), score("flush", [8, 6, 5, 4, 3]) > score("two pair", [9, 4, 14]))
