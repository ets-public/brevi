import sys
from collections import deque
lines = sys.stdin.read().splitlines()
n = int(lines[0])

def parse(s):
    return (ord(s[0]) - 97, int(s[1:]) - 1)

mv = [(1, 2), (2, 1), (-1, 2), (-2, 1), (1, -2), (2, -1), (-1, -2), (-2, -1)]
for line in lines[1:]:
    p = line.split()
    if len(p) < 2:
        continue
    a, b = parse(p[0]), parse(p[1])
    d = {a: 0}
    q = deque([a])
    while q:
        u = q.popleft()
        if u == b:
            break
        for dx, dy in mv:
            v = (u[0] + dx, u[1] + dy)
            if 0 <= v[0] < n and 0 <= v[1] < n and v not in d:
                d[v] = d[u] + 1
                q.append(v)
    print(d.get(b, -1))
