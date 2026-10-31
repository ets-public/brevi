import sys
import heapq

lines = [l.split() for l in sys.stdin if l.strip()]
src, dst = lines[-1][0], lines[-1][1]
adj = {}
for parts in lines[:-1]:
    a, b, w = parts[0], parts[1], int(parts[2])
    adj.setdefault(a, []).append((b, w))
    adj.setdefault(b, []).append((a, w))

# Distances from destination, so the path can be built greedily from the source.
INF = float("inf")
dist = {dst: 0}
pq = [(0, dst)]
while pq:
    d, u = heapq.heappop(pq)
    if d > dist.get(u, INF):
        continue
    for v, w in adj.get(u, []):
        nd = d + w
        if nd < dist.get(v, INF):
            dist[v] = nd
            heapq.heappush(pq, (nd, v))

if src not in dist:
    print("none")
else:
    path = [src]
    cur = src
    while cur != dst:
        best = None
        for v, w in adj.get(cur, []):
            if v in dist and w + dist[v] == dist[cur]:
                if best is None or v < best:
                    best = v
        cur = best
        path.append(cur)
    print(dist[src])
    print("->".join(path))
