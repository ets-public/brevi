import sys
from decimal import Decimal, ROUND_HALF_UP
d = {}
for line in sys.stdin:
    p = line.split()
    if len(p) < 4:
        continue
    d.setdefault(p[1], []).append((int(p[2]), int(p[3])))
def r1(x):
    return format(x.quantize(Decimal('0.1'), rounding=ROUND_HALF_UP), 'f')
for path in sorted(d):
    v = d[path]
    n = len(v)
    ms = sorted(m for _, m in v)
    avg = Decimal(sum(ms)) / n
    p90 = ms[(9 * n + 9) // 10 - 1]
    err = Decimal(100 * sum(1 for s, _ in v if s >= 500)) / n
    print(path, n, r1(avg), p90, r1(err) + '%')
