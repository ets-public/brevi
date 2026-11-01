class Orders:
    def __init__(self):
        self.ids = set()
        self.back = []  # [id, sku, left]

    def known(self, oid):
        return oid in self.ids

    def register(self, oid):
        self.ids.add(oid)

    def add_back(self, oid, sku, left):
        self.back.append([oid, sku, left])

    def fill(self, sku, stock):
        """Fill backorders for sku; returns list of (message, shipped)."""
        out = []
        for b in list(self.back):
            if b[1] != sku:
                continue
            got = stock.take(sku, b[2])
            if got == 0:
                continue
            b[2] -= got
            if b[2] == 0:
                self.back.remove(b)
                out.append(('BACKORDER %s: done' % b[0], got))
            else:
                out.append(('BACKORDER %s: shipped %d, %d left' % (b[0], got, b[2]), got))
        return out

    def waiting(self):
        return sum(b[2] for b in self.back)
