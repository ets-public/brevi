import csv
import sys
from decimal import Decimal


def main():
    reader = csv.reader(sys.stdin)
    header = None
    totals = {}
    for row in reader:
        if not row or all(not c.strip() for c in row):
            continue
        if header is None:
            header = [h.strip() for h in row]
            continue
        rec = dict(zip(header, row))
        item = rec.get("item", "").strip()
        qty = int(rec.get("qty", "0").strip())
        price = Decimal(rec.get("price", "0").strip())
        totals[item] = totals.get(item, Decimal("0")) + qty * price
    items = sorted(totals.items(), key=lambda kv: (-kv[1], kv[0]))
    out = [f"{name}: {val:.2f}" for name, val in items]
    if out:
        sys.stdout.write("\n".join(out) + "\n")


main()
