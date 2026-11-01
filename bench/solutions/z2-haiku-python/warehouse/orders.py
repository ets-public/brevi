class Orders:
    def __init__(self):
        self.ids = set()
        self.backorders = []  # dicts: id, sku, left (in creation order)

    def add_backorder(self, oid, sku, qty):
        self.backorders.append({'id': oid, 'sku': sku, 'left': qty})

    def fill(self, sku, stock):
        """Fill waiting backorders for sku from stock. Returns (messages, units shipped)."""
        msgs = []
        shipped = 0
        for b in self.backorders:
            if b['sku'] != sku:
                continue
            got = stock.take(sku, b['left'])
            if got == 0:
                continue
            shipped += got
            b['left'] -= got
            if b['left'] == 0:
                msgs.append('BACKORDER %s: done' % b['id'])
            else:
                msgs.append('BACKORDER %s: shipped %d, %d left' % (b['id'], got, b['left']))
        self.backorders = [b for b in self.backorders if b['left'] > 0]
        return msgs, shipped

    def waiting(self):
        return sum(b['left'] for b in self.backorders)
