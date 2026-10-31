import sys
from collections import deque

lines = [l.split() for l in sys.stdin.read().splitlines() if l.strip()]
N = int(lines[0][0])


def parse(sq):
    c = ord(sq[0]) - ord('a')
    r = int(sq[1:]) - 1
    return (c, r)


def name(p):
    return chr(ord('a') + p[0]) + str(p[1] + 1)


MOVES = [(1, 2), (2, 1), (2, -1), (1, -2), (-1, -2), (-2, -1), (-2, 1), (-1, 2)]


def bfs(src):
    dist = {src: 0}
    q = deque([src])
    while q:
        c, r = q.popleft()
        for dc, dr in MOVES:
            nc, nr = c + dc, r + dr
            if 0 <= nc < N and 0 <= nr < N and (nc, nr) not in dist:
                dist[(nc, nr)] = dist[(c, r)] + 1
                q.append((nc, nr))
    return dist


cache = {}
for p in lines[1:]:
    if len(p) < 2:
        continue
    src = parse(p[0])
    dst = parse(p[1])
    if src not in cache:
        cache[src] = bfs(src)
    print(cache[src].get(dst, -1))
