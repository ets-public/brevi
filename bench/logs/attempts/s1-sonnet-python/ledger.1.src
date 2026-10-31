import sys, csv
from decimal import Decimal
rows = list(csv.reader(sys.stdin))
months = {}
acc = {}
for r in rows[1:]:
    if len(r) < 3:
        continue
    m = r[0].strip()[:7]
    a = Decimal(r[2].strip())
    mi = months.setdefault(m, [Decimal(0), Decimal(0)])
    if a > 0:
        mi[0] += a
    else:
        mi[1] += -a
    acc[r[1]] = acc.get(r[1], Decimal(0)) + a

def f(x):
    return format(x.quantize(Decimal("0.01")), 'f')

for m in sorted(months):
    i, o = months[m]
    print(m, f(i), f(o), f(i - o))
if acc:
    k = min(acc, key=lambda x: (-acc[x], x))
    print("top", k, f(acc[k]))
