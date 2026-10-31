import sys, csv, re
from decimal import Decimal

lines = sys.stdin.read().split("\n")
lines = [l.rstrip("\r") for l in lines]
rows = 0
skipped = 0
regions = {}
prods = {}
months = {}
date_re = re.compile(r"^\d{4}-\d{2}-\d{2}$")
qty_re = re.compile(r"^[0-9]+$")
price_re = re.compile(r"^[0-9]+(\.[0-9]{1,2})?$")
data = lines[1:]
for rec in csv.reader(data):
    if not rec or (len(rec) == 1 and rec[0].strip() == ""):
        continue
    if len(rec) != 5:
        skipped += 1
        continue
    d, r, p, q, u = rec
    if not (date_re.match(d) and qty_re.match(q) and price_re.match(u)):
        skipped += 1
        continue
    q = int(q)
    cents = q * int(round(Decimal(u) * 100))
    rows += 1
    regions[r] = regions.get(r, 0) + cents
    prods[p] = prods.get(p, 0) + q
    m = d[:7]
    months[m] = months.get(m, 0) + cents

def fmt(c):
    return "%d.%02d" % (c // 100, c % 100)

print("rows", rows)
print("skipped", skipped)
if rows:
    print("regions")
    for r, c in sorted(regions.items(), key=lambda x: (-x[1], x[0])):
        print("  %s %s" % (r, fmt(c)))
    name, qty = sorted(prods.items(), key=lambda x: (-x[1], x[0]))[0]
    print("top_product %s %d" % (name, qty))
    print("months")
    for m in sorted(months):
        print("  %s %s" % (m, fmt(months[m])))
