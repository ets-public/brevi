class Orders:
    """Tracks order IDs and waiting backorders; ships through the Stock."""

    def __init__(self, stock):
        self.stock = stock
        self.ids = set()
        self.backorders = []  # dicts: id, sku, left (in creation order)

    def has(self, oid):
        return oid in self.ids

    def place(self, oid, sku, qty):
        """Record an order. Returns (units shipped, lines to print)."""
        self.ids.add(oid)
        shipped = self.stock.take(sku, qty)
        if shipped == qty:
            lines = ["ORDER %s: shipped %d" % (oid, qty)]
        else:
            lines = ["ORDER %s: shipped %d of %d" % (oid, shipped, qty)]
            self.backorders.append({"id": oid, "sku": sku, "left": qty - shipped})
        return shipped, lines

    def fill(self, sku):
        """Fill waiting backorders for sku, oldest first. Returns (units shipped, lines)."""
        total = 0
        lines = []
        for bo in self.backorders:
            if bo["sku"] != sku:
                continue
            got = self.stock.take(sku, bo["left"])
            if got == 0:
                continue
            bo["left"] -= got
            total += got
            if bo["left"] == 0:
                lines.append("BACKORDER %s: done" % bo["id"])
            else:
                lines.append("BACKORDER %s: shipped %d, %d left" % (bo["id"], got, bo["left"]))
        self.backorders = [bo for bo in self.backorders if bo["left"] > 0]
        return total, lines

    def waiting(self):
        return sum(bo["left"] for bo in self.backorders)
