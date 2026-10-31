import sys, heapq
lines = [l.split() for l in sys.stdin.read().splitlines() if l.strip()]
src, dst = lines[-1][0], lines[-1][1]
adj = {}
for a, b, w in lines[:-1]:
    w = int(w)
    adj.setdefault(a, {})
    adj.setdefault(b, {})
    if b not in adj[a] or adj[a][b] > w:
        adj[a][b] = w
        adj[b][a] = w

def dij(s):
    d = {s: 0}
    h = [(0, s)]
    while h:
        du, u = heapq.heappop(h)
        if du > d.get(u, 1 << 60):
            continue
        for v, w in adj.get(u, {}).items():
            nd = du + w
            if nd < d.get(v, 1 << 60):
                d[v] = nd
                heapq.heappush(h, (nd, v))
    return d

dt = dij(dst)
if src not in dt:
    print("none")
else:
    print(dt[src])
    path = [src]
    u = src
    while u != dst:
        best = None
        for v, w in adj[u].items():
            if v in dt and dt[v] + w == dt[u]:
                if best is None or v < best:
                    best = v
        u = best
        path.append(u)
    print("->".join(path))
