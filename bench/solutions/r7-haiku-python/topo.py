import sys, heapq
from collections import defaultdict

edges = set()
nodes = set()
for line in sys.stdin:
    line = line.strip()
    if not line:
        continue
    a, b = [x.strip() for x in line.split('->')]
    edges.add((a, b))
    nodes.add(a)
    nodes.add(b)

adj = defaultdict(list)
indeg = {n: 0 for n in nodes}
for a, b in edges:
    adj[a].append(b)
    indeg[b] += 1

heap = [n for n in nodes if indeg[n] == 0]
heapq.heapify(heap)
order = []
while heap:
    n = heapq.heappop(heap)
    order.append(n)
    for m in adj[n]:
        indeg[m] -= 1
        if indeg[m] == 0:
            heapq.heappush(heap, m)

if len(order) != len(nodes):
    print("cycle")
else:
    for n in order:
        print(n)
