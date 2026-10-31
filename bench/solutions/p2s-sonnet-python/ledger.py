import sys, csv
from decimal import Decimal
r = csv.reader(sys.stdin)
next(r, None)
months = {}
acc = {}
for row in r:
    if len(row) < 3 or not row[0].strip():
        continue
    m = row[0].strip()[:7]
    a = Decimal(row[2].strip())
    ins, outs = months.get(m, (Decimal(0), Decimal(0)))
    if a > 0:
        ins += a
    else:
        outs += -a
    months[m] = (ins, outs)
    acc[row[1]] = acc.get(row[1], Decimal(0)) + a
f = lambda d: format(d.quantize(Decimal('0.01')), 'f')
for m in sorted(months):
    i, o = months[m]
    print(m, f(i), f(o), f(i - o))
if acc:
    name = sorted(acc.items(), key=lambda kv: (-kv[1], kv[0]))[0]
    print("top", name[0], f(name[1]))
