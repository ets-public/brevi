import sys, csv
from decimal import Decimal
rows = list(csv.reader(sys.stdin))
months = {}
bal = {}
for r in rows[1:]:
    if len(r) < 3:
        continue
    m = r[0].strip()[:7]
    a = Decimal(r[2].strip())
    x = months.setdefault(m, [Decimal(0), Decimal(0)])
    if a > 0:
        x[0] += a
    else:
        x[1] += -a
    bal[r[1]] = bal.get(r[1], Decimal(0)) + a
f = lambda v: format(v, '.2f')
for m in sorted(months):
    i, o = months[m]
    print(m, f(i), f(o), f(i - o))
if bal:
    k = sorted(bal, key=lambda a: (-bal[a], a))[0]
    print("top", k, f(bal[k]))
