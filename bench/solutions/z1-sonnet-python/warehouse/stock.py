class Stock:
    def __init__(self):
        self.lots = []
        self.seq = 0

    def add(self, sku, qty, exp):
        self.seq += 1
        self.lots.append([sku, exp, self.seq, qty])

    def expire(self, today):
        gone = sorted([l for l in self.lots if l[1] < today])
        self.lots = [l for l in self.lots if l[1] >= today]
        return gone

    def take(self, sku, qty):
        got = 0
        for l in sorted((l for l in self.lots if l[0] == sku), key=lambda l: (l[1], l[2])):
            if got >= qty:
                break
            t = min(qty - got, l[3])
            l[3] -= t
            got += t
        self.lots = [l for l in self.lots if l[3] > 0]
        return got

    def summary(self):
        d = {}
        for l in self.lots:
            a = d.setdefault(l[0], [0, 0])
            a[0] += l[3]
            a[1] += 1
        return sorted(d.items())
