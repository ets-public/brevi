import sys
from decimal import Decimal, ROUND_HALF_UP

paths = {}
for line in sys.stdin:
    parts = line.split()
    if len(parts) < 4:
        continue
    method, path, status, ms = parts[0], parts[1], int(parts[2]), int(parts[3])
    p = paths.setdefault(path, {"ms": [], "err": 0})
    p["ms"].append(ms)
    if status >= 500:
        p["err"] += 1


def r1(num, den):
    return (Decimal(num) / Decimal(den)).quantize(Decimal("0.1"), rounding=ROUND_HALF_UP)


for path in sorted(paths):
    p = paths[path]
    ms = sorted(p["ms"])
    n = len(ms)
    avg = r1(sum(ms), n)
    k = (9 * n + 9) // 10
    p90 = ms[k - 1]
    err = r1(p["err"] * 100, n)
    print("%s %d %s %d %s%%" % (path, n, avg, p90, err))
