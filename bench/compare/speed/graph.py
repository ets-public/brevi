import sys, heapq
data = sys.stdin.read().split('\n')
n = int(data[0].split()[0])
adj = [[] for _ in range(n)]
for l in data[1:]:
    if not l: continue
    a, b, w = map(int, l.split())
    adj[a].append((b, w)); adj[b].append((a, w))
INF = 10**9
dist = [INF]*n; dist[0] = 0
h = [(0, 0)]
while h:
    d, u = heapq.heappop(h)
    if d > dist[u]: continue
    for v, w in adj[u]:
        nd = d + w
        if nd < dist[v]:
            dist[v] = nd; heapq.heappush(h, (nd, v))
r = [d for d in dist if d < INF]
print(len(r), sum(r))
