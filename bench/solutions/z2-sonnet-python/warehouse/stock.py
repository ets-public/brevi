class Stock:
    def __init__(self):
        self.lots = []  # [sku, expiry, seq, qty]
        self.seq = 0

    def add(self, sku, qty, expiry):
        self.seq += 1
        self.lots.append([sku, expiry, self.seq, qty])

    def take(self, sku, qty):
        got = 0
        for lot in sorted((l for l in self.lots if l[0] == sku), key=lambda l: (l[1], l[2])):
            if got >= qty:
                break
            k = min(qty - got, lot[3])
            lot[3] -= k
            got += k
        self.lots = [l for l in self.lots if l[3] > 0]
        return got

    def expire(self, today):
        gone = sorted((l for l in self.lots if l[1] < today), key=lambda l: (l[0], l[1], l[2]))
        self.lots = [l for l in self.lots if l[1] >= today]
        return [(l[0], l[3], l[1]) for l in gone]

    def summary(self):
        r = {}
        for l in self.lots:
            q, c = r.get(l[0], (0, 0))
            r[l[0]] = (q + l[3], c + 1)
        return [(k, r[k][0], r[k][1]) for k in sorted(r)]
