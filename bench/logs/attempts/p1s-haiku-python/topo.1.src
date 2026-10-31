import sys
import heapq

indeg = {}
succ = {}
for line in sys.stdin:
    if "->" not in line:
        continue
    a, b = [x.strip() for x in line.split("->", 1)]
    if not a or not b:
        continue
    indeg.setdefault(a, 0)
    indeg.setdefault(b, 0)
    succ.setdefault(a, set())
    if b not in succ[a]:
        succ[a].add(b)
        indeg[b] += 1

heap = [n for n, d in indeg.items() if d == 0]
heapq.heapify(heap)
order = []
while heap:
    n = heapq.heappop(heap)
    order.append(n)
    for m in succ.get(n, ()):
        indeg[m] -= 1
        if indeg[m] == 0:
            heapq.heappush(heap, m)

if len(order) != len(indeg):
    print("cycle")
else:
    for n in order:
        print(n)
