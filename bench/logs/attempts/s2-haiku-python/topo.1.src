import sys
import heapq

succ = {}
indeg = {}
for line in sys.stdin.read().splitlines():
    if "->" not in line:
        continue
    a, b = [x.strip() for x in line.split("->", 1)]
    succ.setdefault(a, set())
    succ.setdefault(b, set())
    indeg.setdefault(a, 0)
    indeg.setdefault(b, 0)
    if b not in succ[a]:
        succ[a].add(b)
        indeg[b] += 1

heap = [n for n in indeg if indeg[n] == 0]
heapq.heapify(heap)
order = []
while heap:
    n = heapq.heappop(heap)
    order.append(n)
    for m in succ[n]:
        indeg[m] -= 1
        if indeg[m] == 0:
            heapq.heappush(heap, m)

if len(order) != len(indeg):
    print("cycle")
else:
    for n in order:
        print(n)
