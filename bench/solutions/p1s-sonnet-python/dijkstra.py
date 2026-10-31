import sys, heapq
lines = [l.split() for l in sys.stdin if l.strip()]
src, dst = lines[-1]
adj = {}
for a, b, w in lines[:-1]:
    w = int(w)
    adj.setdefault(a, []).append((b, w))
    adj.setdefault(b, []).append((a, w))
adj.setdefault(src, [])
adj.setdefault(dst, [])
dist = {dst: 0}
h = [(0, dst)]
while h:
    d, u = heapq.heappop(h)
    if d > dist.get(u, 1 << 60):
        continue
    for v, w in adj[u]:
        nd = d + w
        if nd < dist.get(v, 1 << 60):
            dist[v] = nd
            heapq.heappush(h, (nd, v))
if src not in dist:
    print("none")
else:
    print(dist[src])
    path = [src]
    cur = src
    while cur != dst:
        best = None
        for v, w in adj[cur]:
            if v in dist and w + dist[v] == dist[cur]:
                if best is None or v < best:
                    best = v
        path.append(best)
        cur = best
    print("->".join(path))
