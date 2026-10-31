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

for s, e in merged:
    print(s, e)
total = sum(e - s for s, e in merged)
print("total", total)
