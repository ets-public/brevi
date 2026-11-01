class Stock:
    """Lots of goods. Units are always taken from the earliest expiry first."""

    def __init__(self):
        self.lots = []  # dicts: sku, qty, expiry, seq
        self.seq = 0

    def add(self, sku, qty, expiry):
        self.seq += 1
        self.lots.append({"sku": sku, "qty": qty, "expiry": expiry, "seq": self.seq})

    def discard_before(self, today):
        """Remove lots expiring before `today`. Returns (sku, qty, expiry) tuples in print order."""
        gone = [l for l in self.lots if l["expiry"] < today and l["qty"] > 0]
        self.lots = [l for l in self.lots if l["expiry"] >= today]
        gone.sort(key=lambda l: (l["sku"], l["expiry"], l["seq"]))
        return [(l["sku"], l["qty"], l["expiry"]) for l in gone]

    def _ordered(self, sku):
        lots = [l for l in self.lots if l["sku"] == sku and l["qty"] > 0]
        lots.sort(key=lambda l: (l["expiry"], l["seq"]))
        return lots

    def take(self, sku, qty):
        """Take up to qty units of sku; returns the number actually taken."""
        taken = 0
        for lot in self._ordered(sku):
            if taken == qty:
                break
            n = min(lot["qty"], qty - taken)
            lot["qty"] -= n
            taken += n
        self.lots = [l for l in self.lots if l["qty"] > 0]
        return taken

    def summary(self):
        """List of (sku, units, lots_with_units) sorted by sku."""
        per = {}
        for l in self.lots:
            if l["qty"] <= 0:
                continue
            units, count = per.get(l["sku"], (0, 0))
            per[l["sku"]] = (units + l["qty"], count + 1)
        return [(sku, per[sku][0], per[sku][1]) for sku in sorted(per)]
