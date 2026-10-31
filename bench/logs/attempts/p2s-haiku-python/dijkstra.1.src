import sys
import heapq

lines = [l.split() for l in sys.stdin.read().splitlines() if l.strip()]
edges = lines[:-1]
src, dst = lines[-1][0], lines[-1][1]

adj = {}
for a, b, w in edges:
    w = int(w)
    adj.setdefault(a, []).append((b, w))
    adj.setdefault(b, []).append((a, w))

# distances from dst, so we can walk forward greedily picking smallest names
INF = float('inf')
dt = {dst: 0}
pq = [(0, dst)]
while pq:
    d, u = heapq.heappop(pq)
    if d > dt.get(u, INF):
        continue
    for v, w in adj.get(u, []):
        nd = d + w
        if nd < dt.get(v, INF):
            dt[v] = nd
            heapq.heappush(pq, (nd, v))

if src not in dt:
    print("none")
else:
    total = dt[src]
    path = [src]
    u = src
    while u != dst:
        best = None
        for v, w in adj.get(u, []):
            if v in dt and w + dt[v] == dt[u]:
                if best is None or v < best:
                    best = v
        u = best
        path.append(u)
    print(total)
    print("->".join(path))
