import sys
from decimal import Decimal, ROUND_HALF_UP
from collections import defaultdict

ONE = Decimal("0.1")


def r1(d):
    return str(d.quantize(ONE, rounding=ROUND_HALF_UP))


data = defaultdict(lambda: {"ms": [], "err": 0})
for line in sys.stdin:
    p = line.split()
    if len(p) < 4:
        continue
    path = p[1]
    status = int(p[2])
    ms = int(p[3])
    data[path]["ms"].append(ms)
    if status >= 500:
        data[path]["err"] += 1

for path in sorted(data):
    vals = sorted(data[path]["ms"])
    n = len(vals)
    avg = Decimal(sum(vals)) / Decimal(n)
    k = (9 * n + 9) // 10 - 1
    p90 = vals[k]
    err = Decimal(data[path]["err"]) * 100 / Decimal(n)
    print(f"{path} {n} {r1(avg)} {p90} {r1(err)}%")
