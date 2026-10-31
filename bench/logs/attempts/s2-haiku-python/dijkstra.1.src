import sys
import heapq

lines = [l.split() for l in sys.stdin.read().splitlines() if l.strip()]
src, dst = lines[-1][0], lines[-1][1]

adj = {}
for p in lines[:-1]:
    a, b, w = p[0], p[1], int(p[2])
    adj.setdefault(a, {})
    adj.setdefault(b, {})
    if b not in adj[a] or adj[a][b] > w:
        adj[a][b] = w
    if a not in adj[b] or adj[b][a] > w:
        adj[b][a] = w


def dij(s):
    d = {s: 0}
    pq = [(0, s)]
    while pq:
        du, u = heapq.heappop(pq)
        if du > d[u]:
            continue
        for v, w in adj.get(u, {}).items():
            nd = du + w
            if v not in d or nd < d[v]:
                d[v] = nd
                heapq.heappush(pq, (nd, v))
    return d


dd = dij(dst)
if src not in dd:
    print("none")
else:
    print(dd[src])
    cur = src
    path = [src]
    while cur != dst:
        best = None
        for v, w in adj[cur].items():
            if v in dd and w + dd[v] == dd[cur]:
                if best is None or v < best:
                    best = v
        cur = best
        path.append(cur)
    print("->".join(path))
