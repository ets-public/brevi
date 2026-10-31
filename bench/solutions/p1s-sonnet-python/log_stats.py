import sys
from decimal import Decimal, ROUND_HALF_UP
d = {}
for line in sys.stdin:
    p = line.split()
    if len(p) < 4:
        continue
    d.setdefault(p[1], []).append((int(p[2]), int(p[3])))
def fmt(x):
    return str(x.quantize(Decimal('0.1'), rounding=ROUND_HALF_UP))
for path in sorted(d):
    r = d[path]
    n = len(r)
    ms = sorted(m for _, m in r)
    avg = Decimal(sum(ms)) / n
    p90 = ms[(9 * n + 9) // 10 - 1]
    err = Decimal(100 * sum(1 for s, _ in r if s >= 500)) / n
    print("%s %d %s %d %s%%" % (path, n, fmt(avg), p90, fmt(err)))
