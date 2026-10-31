import sys, csv
from decimal import Decimal
rows = list(csv.reader(sys.stdin))
tot = {}
for r in rows[1:]:
    if len(r) < 3:
        continue
    name = r[0]
    tot[name] = tot.get(name, Decimal(0)) + int(r[1].strip()) * Decimal(r[2].strip())
for name, v in sorted(tot.items(), key=lambda x: (-x[1], x[0])):
    print("%s: %.2f" % (name, v))
