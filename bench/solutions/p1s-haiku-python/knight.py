import sys
from collections import deque

lines = [l.split() for l in sys.stdin if l.strip()]
N = int(lines[0][0])


def parse(sq):
    col = ord(sq[0]) - ord("a")
    row = int(sq[1:]) - 1
    return col, row


MOVES = [(1, 2), (2, 1), (-1, 2), (-2, 1), (1, -2), (2, -1), (-1, -2), (-2, -1)]


def bfs(start):
    dist = {start: 0}
    q = deque([start])
    while q:
        c, r = q.popleft()
        for dc, dr in MOVES:
            nc, nr = c + dc, r + dr
            if 0 <= nc < N and 0 <= nr < N and (nc, nr) not in dist:
                dist[(nc, nr)] = dist[(c, r)] + 1
                q.append((nc, nr))
    return dist


cache = {}
for parts in lines[1:]:
    a = parse(parts[0])
    b = parse(parts[1])
    if a not in cache:
        cache[a] = bfs(a)
    print(cache[a].get(b, -1))
