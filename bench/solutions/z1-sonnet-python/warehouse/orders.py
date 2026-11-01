class Orders:
    def __init__(self):
        self.ids = set()
        self.back = []

    def seen(self, oid):
        return oid in self.ids

    def add(self, oid, sku, qty, shipped):
        self.ids.add(oid)
        if shipped < qty:
            self.back.append([oid, sku, qty - shipped])

    def fill(self, sku, stock, out):
        total = 0
        for b in self.back:
            if b[1] != sku:
                continue
            got = stock.take(sku, b[2])
            if got:
                b[2] -= got
                total += got
                if b[2] == 0:
                    out.append('BACKORDER %s: done' % b[0])
                else:
                    out.append('BACKORDER %s: shipped %d, %d left' % (b[0], got, b[2]))
        self.back = [b for b in self.back if b[2] > 0]
        return total

    def waiting(self):
        return sum(b[2] for b in self.back)
