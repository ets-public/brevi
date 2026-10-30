import sys
from collections import deque

tokens = sys.stdin.read().split()
N = int(tokens[0])
MOVES = [(1, 2), (2, 1), (-1, 2), (-2, 1), (1, -2), (2, -1), (-1, -2), (-2, -1)]
cache = {}

def parse(sq):
    col = ord(sq[0]) - ord('a')
    row = int(sq[1:]) - 1
    return col, row

def bfs(src):
    if src in cache:
        return cache[src]
    dist = {src: 0}
    q = deque([src])
    while q:
        c, r = q.popleft()
        for dc, dr in MOVES:
            nc, nr = c + dc, r + dr
            if 0 <= nc < N and 0 <= nr < N and (nc, nr) not in dist:
                dist[(nc, nr)] = dist[(c, r)] + 1
                q.append((nc, nr))
    cache[src] = dist
    return dist

idx = 1
out = []
while idx + 1 < len(tokens):
    a, b = tokens[idx], tokens[idx + 1]
    idx += 2
    try:
        src, dst = parse(a), parse(b)
    except ValueError:
        out.append('-1')
        continue
    if not (0 <= src[0] < N and 0 <= src[1] < N and 0 <= dst[0] < N and 0 <= dst[1] < N):
        out.append('-1')
        continue
    out.append(str(bfs(src).get(dst, -1)))
print('\n'.join(out))
