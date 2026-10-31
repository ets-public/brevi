import sys
from collections import deque

lines = [l.split() for l in sys.stdin.read().splitlines() if l.strip()]
N = int(lines[0][0])


def parse(sq):
    c = ord(sq[0]) - ord('a')
    r = int(sq[1:]) - 1
    return c, r


def ok(c, r):
    return 0 <= c < N and 0 <= r < N


MOVES = [(1, 2), (2, 1), (-1, 2), (-2, 1), (1, -2), (2, -1), (-1, -2), (-2, -1)]


def bfs(src, dst):
    if src == dst:
        return 0
    dist = {src: 0}
    q = deque([src])
    while q:
        c, r = q.popleft()
        d = dist[(c, r)]
        for dc, dr in MOVES:
            nc, nr = c + dc, r + dr
            if ok(nc, nr) and (nc, nr) not in dist:
                if (nc, nr) == dst:
                    return d + 1
                dist[(nc, nr)] = d + 1
                q.append((nc, nr))
    return -1


for parts in lines[1:]:
    if len(parts) < 2:
        continue
    a = parse(parts[0])
    b = parse(parts[1])
    if not ok(*a) or not ok(*b):
        print(-1)
    else:
        print(bfs(a, b))
