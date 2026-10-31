import sys
from fractions import Fraction
from decimal import Decimal, ROUND_HALF_UP
d = {}
for line in sys.stdin:
    p = line.split()
    if len(p) < 4:
        continue
    d.setdefault(p[1], []).append((int(p[2]), int(p[3])))

def fmt(fr):
    x = Decimal(fr.numerator) / Decimal(fr.denominator)
    return str(x.quantize(Decimal('0.1'), rounding=ROUND_HALF_UP))

for path in sorted(d):
    rs = d[path]
    n = len(rs)
    ms = sorted(m for _, m in rs)
    avg = Fraction(sum(ms), n)
    p90 = ms[(9 * n + 9) // 10 - 1]
    err = Fraction(100 * sum(1 for s, _ in rs if s >= 500), n)
    print(path, n, fmt(avg), p90, fmt(err) + "%")
