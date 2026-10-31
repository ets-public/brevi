import sys
from collections import deque
lines = [l.split() for l in sys.stdin if l.strip()]
n = int(lines[0][0])

def sq(s):
    return (ord(s[0]) - 97, int(s[1:]) - 1)

for p in lines[1:]:
    a, b = sq(p[0]), sq(p[1])
    ok = lambda s: 0 <= s[0] < n and 0 <= s[1] < n
    if not (ok(a) and ok(b)):
        print(-1)
        continue
    d = {a: 0}
    q = deque([a])
    while q and b not in d:
        x, y = q.popleft()
        for dx, dy in ((1,2),(2,1),(-1,2),(-2,1),(1,-2),(2,-1),(-1,-2),(-2,-1)):
            t = (x + dx, y + dy)
            if ok(t) and t not in d:
                d[t] = d[(x, y)] + 1
                q.append(t)
    print(d.get(b, -1))
