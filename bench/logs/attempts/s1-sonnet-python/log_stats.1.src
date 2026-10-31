import sys
d = {}
for line in sys.stdin:
    p = line.split()
    if len(p) < 4:
        continue
    d.setdefault(p[1], []).append((int(p[2]), int(p[3])))
for path in sorted(d):
    v = d[path]
    n = len(v)
    ms = sorted(x[1] for x in v)
    s = sum(ms)
    avg = (s * 20 + n) // (2 * n)
    p90 = ms[(9 * n + 9) // 10 - 1]
    err = sum(1 for x in v if x[0] >= 500)
    ep = (err * 2000 + n) // (2 * n)
    print("%s %d %d.%d %d %d.%d%%" % (path, n, avg // 10, avg % 10, p90, ep // 10, ep % 10))
