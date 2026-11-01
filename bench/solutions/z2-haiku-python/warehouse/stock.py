class Stock:
    def __init__(self):
        self.lots = []  # dicts: sku, qty, expiry (date), seq (receive order)
        self.seq = 0

    def add(self, sku, qty, expiry):
        self.seq += 1
        self.lots.append({'sku': sku, 'qty': qty, 'expiry': expiry, 'seq': self.seq})

    def discard_before(self, today):
        gone = [lot for lot in self.lots if lot['expiry'] < today]
        self.lots = [lot for lot in self.lots if lot['expiry'] >= today]
        gone.sort(key=lambda lot: (lot['sku'], lot['expiry'], lot['seq']))
        return gone

    def take(self, sku, qty):
        """Ship up to qty units of sku, earliest expiry first. Returns units shipped."""
        mine = sorted((lot for lot in self.lots if lot['sku'] == sku),
                      key=lambda lot: (lot['expiry'], lot['seq']))
        got = 0
        for lot in mine:
            if got == qty:
                break
            x = min(lot['qty'], qty - got)
            lot['qty'] -= x
            got += x
        self.lots = [lot for lot in self.lots if lot['qty'] > 0]
        return got

    def summary(self):
        qty = {}
        count = {}
        for lot in self.lots:
            if lot['qty'] > 0:
                qty[lot['sku']] = qty.get(lot['sku'], 0) + lot['qty']
                count[lot['sku']] = count.get(lot['sku'], 0) + 1
        return [(sku, qty[sku], count[sku]) for sku in sorted(qty)]
