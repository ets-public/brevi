import sys, csv
from decimal import Decimal
r = csv.reader(sys.stdin)
next(r, None)
tot = {}
for row in r:
    if len(row) < 3:
        continue
    name = row[0]
    tot[name] = tot.get(name, Decimal(0)) + int(row[1].strip()) * Decimal(row[2].strip())
for name, v in sorted(tot.items(), key=lambda kv: (-kv[1], kv[0])):
    print("%s: %s" % (name, format(v.quantize(Decimal('0.01')), 'f')))
