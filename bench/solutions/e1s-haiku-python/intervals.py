import sys

ivs = []
for line in sys.stdin:
    p = line.split()
    if len(p) < 2:
        continue
    ivs.append((int(p[0]), int(p[1])))

ivs.sort()
merged = []
for s, e in ivs:
    if merged and s <= merged[-1][1]:
        if e > merged[-1][1]:
            merged[-1][1] = e
    else:
        merged.append([s, e])

out = []
total = 0
for s, e in merged:
    out.append("%d %d" % (s, e))
    total += e - s
out.append("total %d" % total)
print("\n".join(out))
