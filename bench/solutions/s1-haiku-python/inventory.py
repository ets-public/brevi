import sys
import csv
from decimal import Decimal

text = sys.stdin.read()
rows = csv.reader(text.splitlines())
header = next(rows, None)

totals = {}
for row in rows:
    if len(row) < 3:
        continue
    item = row[0].strip()
    qty = int(row[1].strip())
    price = Decimal(row[2].strip())
    totals[item] = totals.get(item, Decimal("0")) + qty * price

items = sorted(totals.items(), key=lambda kv: (-kv[1], kv[0]))
for item, val in items:
    print(f"{item}: {val.quantize(Decimal('0.01'))}")
