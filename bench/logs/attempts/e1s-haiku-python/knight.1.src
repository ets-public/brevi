import sys
from collections import deque

lines = sys.stdin.read().splitlines()
N = int(lines[0].strip())
MOVES = [(1, 2), (2, 1), (2, -1), (1, -2), (-1, -2), (-2, -1), (-2, 1), (-1, 2)]


def parse(sq):
    sq = sq.strip()
    c = ord(sq[0].lower()) - ord("a")
    r = int(sq[1:]) - 1
    return (c, r)


cache = {}


def bfs(src):
    if src in cache:
        return cache[src]
    dist = {src: 0}
    dq = deque([src])
    while dq:
        c, r = dq.popleft()
        for dc, dr in MOVES:
            nc, nr = c + dc, r + dr
            if 0 <= nc < N and 0 <= nr < N and (nc, nr) not in dist:
                dist[(nc, nr)] = dist[(c, r)] + 1
                dq.append((nc, nr))
    cache[src] = dist
    return dist


out = []
for line in lines[1:]:
    p = line.split()
    if len(p) < 2:
        continue
    a = parse(p[0])
    b = parse(p[1])
    d = bfs(a).get(b, -1)
    out.append(str(d))

if out:
    print("\n".join(out))
