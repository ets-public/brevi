import sys
from collections import deque
lines = sys.stdin.read().split('\n')
n = int(lines[0].strip())
def sq(s):
    return (ord(s[0]) - 97, int(s[1:]) - 1)
def ok(p):
    return 0 <= p[0] < n and 0 <= p[1] < n
for line in lines[1:]:
    p = line.split()
    if len(p) < 2:
        continue
    a, b = sq(p[0]), sq(p[1])
    if not ok(a) or not ok(b):
        print(-1)
        continue
    d = {a: 0}
    q = deque([a])
    while q:
        u = q.popleft()
        if u == b:
            break
        for dx, dy in ((1,2),(2,1),(-1,2),(-2,1),(1,-2),(2,-1),(-1,-2),(-2,-1)):
            v = (u[0] + dx, u[1] + dy)
            if ok(v) and v not in d:
                d[v] = d[u] + 1
                q.append(v)
    print(d.get(b, -1))
