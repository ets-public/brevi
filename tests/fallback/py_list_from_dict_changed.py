# A list taken out of a dict, then changed with append or pop: in Python it is the dict's own list. The
# typed build made a copy (its rows were shared, the list itself was not), so the dict did not change.
# Found by the DeepSeek release test (warehouse backorders, 9 Nov 2026). It must give CPython's output.
class Orders:
    def __init__(self):
        self.waiting = {}

    def add(self, oid, sku, qty):
        lst = self.waiting.get(sku)
        if lst is None:
            self.waiting[sku] = [[oid, qty]]
        else:
            lst.append([oid, qty])

    def backorders_for(self, sku):
        return self.waiting.get(sku, [])


def fill(book, sku, have):
    lst = book.backorders_for(sku)
    i = 0
    while i < len(lst):
        oid, need = lst[i]
        got = min(have, need)
        if got == 0:
            break
        have -= got
        if got == need:
            print("done", oid)
            lst.pop(i)
        else:
            lst[i][1] = need - got
            print("partial", oid, need - got)
            break
    print(book.waiting)


book = Orders()
book.add("x1", "tea", 5)
book.add("x2", "tea", 3)
book.add("x3", "coffee", 1)
fill(book, "tea", 6)
fill(book, "tea", 6)
