import sys
from decimal import Decimal, ROUND_HALF_UP

paths = {}
for line in sys.stdin.read().splitlines():
    p = line.split()
    if len(p) < 4:
        continue
    method, path, status, ms = p[0], p[1], int(p[2]), int(p[3])
    paths.setdefault(path, []).append((status, ms))


def r1(x):
    return Decimal(x).quantize(Decimal("0.1"), rounding=ROUND_HALF_UP)


for path in sorted(paths):
    entries = paths[path]
    n = len(entries)
    times = sorted(ms for _, ms in entries)
    avg = r1(Decimal(sum(times)) / Decimal(n))
    rank = (9 * n + 9) // 10
    p90 = times[rank - 1]
    errs = sum(1 for s, _ in entries if s >= 500)
    err_pct = r1(Decimal(errs) * 100 / Decimal(n))
    print(f"{path} {n} {avg} {p90} {err_pct}%")
