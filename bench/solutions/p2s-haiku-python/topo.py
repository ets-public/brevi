import sys
import heapq

indeg = {}
adj = {}
for line in sys.stdin:
    parts = line.split()
    if len(parts) < 3:
        continue
    a, b = parts[0], parts[2]
    indeg.setdefault(a, 0)
    indeg.setdefault(b, 0)
    adj.setdefault(a, [])
    adj.setdefault(b, [])
    adj[a].append(b)
    indeg[b] += 1

heap = [n for n in indeg if indeg[n] == 0]
heapq.heapify(heap)
order = []
while heap:
    n = heapq.heappop(heap)
    order.append(n)
    for m in adj[n]:
        indeg[m] -= 1
        if indeg[m] == 0:
            heapq.heappush(heap, m)

if len(order) < len(indeg):
    print("cycle")
else:
    for n in order:
        print(n)
