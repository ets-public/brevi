import sys
from collections import deque

def main():
    lines = [l.split() for l in sys.stdin.read().splitlines()]
    lines = [l for l in lines if l]
    if not lines:
        return
    n = int(lines[0][0])
    moves = [(1, 2), (2, 1), (-1, 2), (-2, 1), (1, -2), (2, -1), (-1, -2), (-2, -1)]
    cache = {}

    def bfs(src):
        if src in cache:
            return cache[src]
        dist = {src: 0}
        q = deque([src])
        while q:
            c, r = q.popleft()
            for dc, dr in moves:
                nc, nr = c + dc, r + dr
                if 0 <= nc < n and 0 <= nr < n and (nc, nr) not in dist:
                    dist[(nc, nr)] = dist[(c, r)] + 1
                    q.append((nc, nr))
        cache[src] = dist
        return dist

    def parse(sq):
        return (ord(sq[0]) - ord('a'), int(sq[1:]) - 1)

    out = []
    for parts in lines[1:]:
        a, b = parse(parts[0]), parse(parts[1])
        out.append(str(bfs(a).get(b, -1)))
    if out:
        sys.stdout.write('\n'.join(out) + '\n')

main()
