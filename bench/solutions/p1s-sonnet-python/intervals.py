import sys
iv = []
for line in sys.stdin:
    p = line.split()
    if len(p) >= 2:
        iv.append((int(p[0]), int(p[1])))
iv.sort()
res = []
for s, e in iv:
    if res and s <= res[-1][1]:
        if e > res[-1][1]:
            res[-1][1] = e
    else:
        res.append([s, e])
for s, e in res:
    print(s, e)
print("total", sum(e - s for s, e in res))
