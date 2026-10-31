import sys
from fractions import Fraction
from collections import defaultdict


def round1(fr):
    # round a non-negative Fraction to 1 decimal, half up; return string
    n = int(fr * 10 + Fraction(1, 2))
    return "%d.%d" % (n // 10, n % 10)


data = defaultdict(list)
for line in sys.stdin:
    p = line.split()
    if len(p) < 4:
        continue
    method, path, status, ms = p[0], p[1], int(p[2]), int(p[3])
    data[path].append((ms, status))

out = []
for path in sorted(data):
    entries = data[path]
    n = len(entries)
    times = sorted(t for t, _ in entries)
    total = sum(times)
    avg = round1(Fraction(total, n))
    rank = (9 * n + 9) // 10
    p90 = times[rank - 1]
    errs = sum(1 for _, s in entries if s >= 500)
    err = round1(Fraction(errs * 100, n)) + "%"
    out.append("%s %d %s %d %s" % (path, n, avg, p90, err))

if out:
    print("\n".join(out))
