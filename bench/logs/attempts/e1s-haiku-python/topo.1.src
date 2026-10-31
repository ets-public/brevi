import sys
import heapq

adj = {}
indeg = {}
nodes = set()
for line in sys.stdin:
    if "->" not in line:
        continue
    a, b = line.split("->", 1)
    a = a.strip()
    b = b.strip()
    if not a or not b:
        continue
    nodes.add(a)
    nodes.add(b)
    adj.setdefault(a, set())
    if b not in adj[a]:
        adj[a].add(b)
        indeg[b] = indeg.get(b, 0) + 1
    indeg.setdefault(a, indeg.get(a, 0))

heap = [n for n in nodes if indeg.get(n, 0) == 0]
heapq.heapify(heap)
order = []
while heap:
    u = heapq.heappop(heap)
    order.append(u)
    for v in adj.get(u, ()):
        indeg[v] -= 1
        if indeg[v] == 0:
            heapq.heappush(heap, v)

if len(order) != len(nodes):
    print("cycle")
else:
    print("\n".join(order))
