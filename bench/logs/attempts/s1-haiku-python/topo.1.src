import sys
import heapq
from collections import defaultdict

nodes = set()
succ = defaultdict(set)
indeg = defaultdict(int)

for line in sys.stdin:
    if "->" not in line:
        continue
    a, b = [x.strip() for x in line.split("->", 1)]
    if not a or not b:
        continue
    nodes.add(a)
    nodes.add(b)
    if b not in succ[a]:
        succ[a].add(b)
        indeg[b] += 1

for n in nodes:
    indeg.setdefault(n, 0)

heap = [n for n in nodes if indeg[n] == 0]
heapq.heapify(heap)
order = []
while heap:
    n = heapq.heappop(heap)
    order.append(n)
    for m in succ[n]:
        indeg[m] -= 1
        if indeg[m] == 0:
            heapq.heappush(heap, m)

if len(order) != len(nodes):
    print("cycle")
else:
    for n in order:
        print(n)
