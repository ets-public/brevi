import sys
from decimal import Decimal, ROUND_HALF_UP

ONE = Decimal("0.1")
data = {}
for line in sys.stdin:
    parts = line.split()
    if len(parts) < 4:
        continue
    method, path, status, ms = parts[0], parts[1], int(parts[2]), int(parts[3])
    data.setdefault(path, []).append((status, ms))

for path in sorted(data):
    rows = data[path]
    n = len(rows)
    times = sorted(ms for _, ms in rows)
    total = sum(times)
    avg = (Decimal(total) / Decimal(n)).quantize(ONE, rounding=ROUND_HALF_UP)
    k = (9 * n + 9) // 10
    p90 = times[k - 1]
    errs = sum(1 for s, _ in rows if s >= 500)
    err = (Decimal(100 * errs) / Decimal(n)).quantize(ONE, rounding=ROUND_HALF_UP)
    print("%s %d %s %d %s%%" % (path, n, avg, p90, err))
