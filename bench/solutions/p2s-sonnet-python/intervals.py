import sys
iv = []
for line in sys.stdin:
    p = line.split()
    if len(p) >= 2:
        iv.append((int(p[0]), int(p[1])))
iv.sort()
m = []
for s, e in iv:
    if m and s <= m[-1][1]:
        if e > m[-1][1]:
            m[-1][1] = e
    else:
        m.append([s, e])
for s, e in m:
    print(s, e)
print("total", sum(e - s for s, e in m))
