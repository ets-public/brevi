import sys
import csv
from decimal import Decimal

rows = csv.reader(sys.stdin)
header = next(rows, None)
totals = {}
for r in rows:
    if len(r) < 3:
        continue
    item, qty, price = r[0], r[1], r[2]
    cents = int((Decimal(price.strip()) * 100).to_integral_value())
    value = int(qty.strip()) * cents
    totals[item] = totals.get(item, 0) + value

items = sorted(totals.items(), key=lambda kv: (-kv[1], kv[0]))
for item, v in items:
    sign = "-" if v < 0 else ""
    v = abs(v)
    print(f"{item}: {sign}{v // 100}.{v % 100:02d}")
