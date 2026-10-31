import sys
import csv
from decimal import Decimal

reader = csv.reader(sys.stdin)
header = next(reader, None)
totals = {}
for row in reader:
    if len(row) < 3:
        continue
    item = row[0].strip()
    qty = int(row[1].strip())
    price = Decimal(row[2].strip())
    cents = int((price * 100).to_integral_value()) * qty
    totals[item] = totals.get(item, 0) + cents

items = sorted(totals.items(), key=lambda kv: (-kv[1], kv[0]))
for item, cents in items:
    sign = "-" if cents < 0 else ""
    c = abs(cents)
    print("%s: %s%d.%02d" % (item, sign, c // 100, c % 100))
