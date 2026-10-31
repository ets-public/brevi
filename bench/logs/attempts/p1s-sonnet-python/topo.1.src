import sys, heapq
adj = {}
indeg = {}
for line in sys.stdin:
    if '->' not in line:
        continue
    a, b = [x.strip() for x in line.split('->')]
    indeg.setdefault(a, 0)
    indeg.setdefault(b, 0)
    adj.setdefault(a, set())
    if b not in adj[a]:
        adj[a].add(b)
        indeg[b] += 1
h = [n for n, d in indeg.items() if d == 0]
heapq.heapify(h)
out = []
while h:
    n = heapq.heappop(h)
    out.append(n)
    for m in adj.get(n, ()):
        indeg[m] -= 1
        if indeg[m] == 0:
            heapq.heappush(h, m)
if len(out) < len(indeg):
    print("cycle")
else:
    print("\n".join(out))
