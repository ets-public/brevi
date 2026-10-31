import sys
import heapq

lines = [l.split() for l in sys.stdin if l.strip()]
edges = lines[:-1]
src, dst = lines[-1][0], lines[-1][1]

adj = {}
for a, b, w in edges:
    w = int(w)
    adj.setdefault(a, []).append((b, w))
    adj.setdefault(b, []).append((a, w))

INF = float('inf')
dist = {src: 0}
pq = [(0, src)]
while pq:
    d, u = heapq.heappop(pq)
    if d > dist.get(u, INF):
        continue
    for v, w in adj.get(u, []):
        nd = d + w
        if nd < dist.get(v, INF):
            dist[v] = nd
            heapq.heappush(pq, (nd, v))

if dst not in dist:
    print("none")
else:
    # Build lexicographically smallest shortest path from src to dst.
    # Compute distances from dst, then greedily pick smallest next node.
    rdist = {dst: 0}
    pq = [(0, dst)]
    while pq:
        d, u = heapq.heappop(pq)
        if d > rdist.get(u, INF):
            continue
        for v, w in adj.get(u, []):
            nd = d + w
            if nd < rdist.get(v, INF):
                rdist[v] = nd
                heapq.heappush(pq, (nd, v))
    total = dist[dst]
    path = [src]
    cur = src
    cur_d = 0
    while cur != dst:
        best = None
        for v, w in adj.get(cur, []):
            if cur_d + w + rdist.get(v, INF) == total:
                if best is None or v < best:
                    best = v
        if best is None:
            break
        for v, w in adj.get(cur, []):
            if v == best:
                cur_d += w
                break
        cur = best
        path.append(cur)
    print(total)
    print("->".join(path))
