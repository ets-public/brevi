import sys
from collections import defaultdict

data = defaultdict(lambda: ([], [0]))  # path -> (ms list, [error count])
for line in sys.stdin:
    parts = line.split()
    if not parts:
        continue
    method, path, status, ms = parts[0], parts[1], int(parts[2]), parts[3]
    ms_val = int(ms) if ms.lstrip('-').isdigit() else float(ms)
    times, errs = data[path]
    times.append(ms_val)
    if status >= 500:
        errs[0] += 1

for path in sorted(data):
    times, errs = data[path]
    n = len(times)
    times_sorted = sorted(times)
    avg = sum(times) / n
    rank = (9 * n + 9) // 10  # ceil(0.9 * n)
    p90 = times_sorted[rank - 1]
    err_pct = errs[0] * 100.0 / n
    print(f"{path} {n} {avg:.1f} {p90} {err_pct:.1f}%")
