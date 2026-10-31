import sys, heapq
succ = {}
indeg = {}
for line in sys.stdin:
    if '->' not in line:
        continue
    a, b = [x.strip() for x in line.split('->')]
    succ.setdefault(a, set())
    succ.setdefault(b, set())
    indeg.setdefault(a, 0)
    indeg.setdefault(b, 0)
    if b not in succ[a]:
        succ[a].add(b)
        indeg[b] += 1
h = [n for n in indeg if indeg[n] == 0]
heapq.heapify(h)
out = []
while h:
    n = heapq.heappop(h)
    out.append(n)
    for m in succ[n]:
        indeg[m] -= 1
        if indeg[m] == 0:
            heapq.heappush(h, m)
if len(out) < len(indeg):
    print("cycle")
else:
    print("\n".join(out))
