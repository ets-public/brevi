import sys
import csv
from decimal import Decimal, ROUND_HALF_UP


def main():
    reader = csv.reader(sys.stdin)
    header = next(reader, None)
    totals = {}
    for row in reader:
        if not row or all(not f.strip() for f in row):
            continue
        item = row[0]
        qty = int(row[1].strip())
        price = Decimal(row[2].strip())
        totals[item] = totals.get(item, Decimal(0)) + qty * price
    items = sorted(totals.items(), key=lambda kv: (-kv[1], kv[0]))
    out = []
    for name, val in items:
        v = val.quantize(Decimal("0.01"), rounding=ROUND_HALF_UP)
        out.append(f"{name}: {v:.2f}")
    print("\n".join(out))


main()
