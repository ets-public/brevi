import sys
from decimal import Decimal
months = {}
acc = {}
first = True
for line in sys.stdin:
    line = line.strip()
    if not line:
        continue
    if first:
        first = False
        continue
    d, a, m = [x.strip() for x in line.split(',')]
    m = Decimal(m)
    mo = d[:7]
    i, o = months.get(mo, (Decimal(0), Decimal(0)))
    if m > 0:
        i += m
    else:
        o += -m
    months[mo] = (i, o)
    acc[a] = acc.get(a, Decimal(0)) + m
for mo in sorted(months):
    i, o = months[mo]
    print("%s %.2f %.2f %.2f" % (mo, i, o, i - o))
if acc:
    best = sorted(acc.items(), key=lambda x: (-x[1], x[0]))[0]
    print("top %s %.2f" % best)
