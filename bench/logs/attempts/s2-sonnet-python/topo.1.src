import sys, heapq
nodes = set()
edges = set()
for line in sys.stdin:
    if '->' not in line:
        continue
    a, b = [x.strip() for x in line.split('->')]
    nodes.add(a); nodes.add(b)
    edges.add((a, b))
adj = {n: [] for n in nodes}
indeg = {n: 0 for n in nodes}
for a, b in edges:
    adj[a].append(b)
    indeg[b] += 1
h = [n for n in nodes if indeg[n] == 0]
heapq.heapify(h)
out = []
while h:
    u = heapq.heappop(h)
    out.append(u)
    for v in adj[u]:
        indeg[v] -= 1
        if indeg[v] == 0:
            heapq.heappush(h, v)
if len(out) < len(nodes):
    print("cycle")
else:
    print("\n".join(out))
