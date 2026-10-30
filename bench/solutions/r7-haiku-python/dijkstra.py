import sys
import heapq
from collections import defaultdict

rows = [l.split() for l in sys.stdin if l.strip()]
frm, to = rows[-1][0], rows[-1][1]
adj = defaultdict(list)
for r in rows[:-1]:
    a, b, w = r[0], r[1], int(r[2])
    adj[a].append((b, w))
    adj[b].append((a, w))

# shortest distances to the target
dist = {to: 0}
heap = [(0, to)]
while heap:
    d, u = heapq.heappop(heap)
    if d > dist.get(u, float('inf')):
        continue
    for v, w in adj[u]:
        nd = d + w
        if nd < dist.get(v, float('inf')):
            dist[v] = nd
            heapq.heappush(heap, (nd, v))

if frm not in dist:
    print("none")
else:
    path = [frm]
    cur = frm
    while cur != to:
        nxt = None
        for v, w in sorted(adj[cur]):
            if v in dist and w + dist[v] == dist[cur]:
                nxt = v
                break
        cur = nxt
        path.append(cur)
    print(dist[frm])
    print('->'.join(path))
