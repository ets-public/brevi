import sys
import csv
from decimal import Decimal

totals = {}
reader = csv.reader(sys.stdin)
header = next(reader, None)
for row in reader:
    if len(row) < 3:
        continue
    item = row[0]
    qty = int(row[1].strip())
    price = Decimal(row[2].strip())
    totals[item] = totals.get(item, Decimal("0")) + qty * price

for item, val in sorted(totals.items(), key=lambda kv: (-kv[1], kv[0])):
    print(f"{item}: {val:.2f}")
