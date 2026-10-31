import sys, csv
from decimal import Decimal
rows = list(csv.reader(sys.stdin))
tot = {}
for r in rows[1:]:
    if len(r) < 3:
        continue
    tot[r[0]] = tot.get(r[0], Decimal(0)) + int(r[1].strip()) * Decimal(r[2].strip())
for k, v in sorted(tot.items(), key=lambda x: (-x[1], x[0])):
    print("%s: %s" % (k, format(v.quantize(Decimal("0.01")), 'f')))
