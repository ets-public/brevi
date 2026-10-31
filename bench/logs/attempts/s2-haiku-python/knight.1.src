import sys
from collections import deque

lines = [l.split() for l in sys.stdin.read().splitlines() if l.strip()]
N = int(lines[0][0])


def parse(sq):
    c = ord(sq[0]) - ord('a')
    r = int(sq[1:]) - 1
    return (c, r)


MOVES = [(1, 2), (2, 1), (2, -1), (1, -2), (-1, -2), (-2, -1), (-2, 1), (-1, 2)]

for p in lines[1:]:
    s = parse(p[0])
    t = parse(p[1])
    dist = {s: 0}
    q = deque([s])
    while q:
        cur = q.popleft()
        if cur == t:
            break
        for dc, dr in MOVES:
            nc, nr = cur[0] + dc, cur[1] + dr
            if 0 <= nc < N and 0 <= nr < N and (nc, nr) not in dist:
                dist[(nc, nr)] = dist[cur] + 1
                q.append((nc, nr))
    print(dist.get(t, -1))
