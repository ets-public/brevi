import sys
from collections import deque


def main():
    lines = [l.split() for l in sys.stdin.read().splitlines() if l.strip()]
    if not lines:
        return
    n = int(lines[0][0])
    cache = {}

    def bfs(src):
        dist = {src: 0}
        q = deque([src])
        while q:
            c, r = q.popleft()
            for dc, dr in ((1, 2), (2, 1), (-1, 2), (-2, 1),
                           (1, -2), (2, -1), (-1, -2), (-2, -1)):
                nc, nr = c + dc, r + dr
                if 0 <= nc < n and 0 <= nr < n and (nc, nr) not in dist:
                    dist[(nc, nr)] = dist[(c, r)] + 1
                    q.append((nc, nr))
        return dist

    def parse(s):
        c = ord(s[0]) - ord("a")
        r = int(s[1:]) - 1
        return (c, r)

    out = []
    for parts in lines[1:]:
        if len(parts) < 2:
            continue
        try:
            a = parse(parts[0])
            b = parse(parts[1])
        except (ValueError, IndexError):
            out.append("-1")
            continue
        if not (0 <= a[0] < n and 0 <= a[1] < n and 0 <= b[0] < n and 0 <= b[1] < n):
            out.append("-1")
            continue
        if a not in cache:
            cache[a] = bfs(a)
        out.append(str(cache[a].get(b, -1)))
    if out:
        sys.stdout.write("\n".join(out) + "\n")


main()
