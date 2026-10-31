import sys
from collections import deque
lines = sys.stdin.read().split('\n')
n = int(lines[0])
def sq(s):
    return (ord(s[0]) - 97, int(s[1:]) - 1)
M = [(1,2),(2,1),(-1,2),(-2,1),(1,-2),(2,-1),(-1,-2),(-2,-1)]
for line in lines[1:]:
    p = line.split()
    if len(p) < 2:
        continue
    a, b = sq(p[0]), sq(p[1])
    d = {a: 0}
    q = deque([a])
    while q and b not in d:
        x, y = q.popleft()
        for dx, dy in M:
            nx, ny = x + dx, y + dy
            if 0 <= nx < n and 0 <= ny < n and (nx, ny) not in d:
                d[(nx, ny)] = d[(x, y)] + 1
                q.append((nx, ny))
    print(d.get(b, -1))
