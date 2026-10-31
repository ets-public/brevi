import sys

ivs = []
for line in sys.stdin:
    parts = line.split()
    if len(parts) < 2:
        continue
    ivs.append((int(parts[0]), int(parts[1])))

ivs.sort()
merged = []
for s, e in ivs:
    if merged and s <= merged[-1][1]:
        if e > merged[-1][1]:
            merged[-1][1] = e
    else:
        merged.append([s, e])

total = 0
for s, e in merged:
    print(s, e)
    total += e - s
print("total", total)
