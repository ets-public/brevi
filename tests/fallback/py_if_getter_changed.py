# A function returning one of two dict items by a condition, its result then changed: Python changes
# the dict's list; the typed build changed a copy. Found by the copy-versus-alias sweep (9 Nov 2026).
book = {"bids": [[10, 1]], "asks": [[11, 2]]}
def side(s):
    return book["bids"] if s == "b" else book["asks"]
lvl = side("b")
lvl.append([9, 3])
lvl.sort(key=lambda x: -x[0])
print(book)
