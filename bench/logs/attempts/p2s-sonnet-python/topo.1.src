import sys, heapq
nodes = set()
edges = set()
for line in sys.stdin:
    if '->' not in line:
        continue
    a, b = [x.strip() for x in line.split('->', 1)]
    nodes.add(a)
    nodes.add(b)
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
    n = heapq.heappop(h)
    out.append(n)
    for m in adj[n]:
        indeg[m] -= 1
        if indeg[m] == 0:
            heapq.heappush(h, m)
if len(out) < len(nodes):
    print("cycle")
else:
    print("\n".join(out))
