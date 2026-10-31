import heapq
import sys


def main():
    adj = {}
    indeg = {}
    for line in sys.stdin:
        if "->" not in line:
            continue
        a, b = (x.strip() for x in line.split("->", 1))
        if not a or not b:
            continue
        for n in (a, b):
            indeg.setdefault(n, 0)
            adj.setdefault(n, set())
        if b not in adj[a]:
            adj[a].add(b)
            indeg[b] += 1
    heap = [n for n, d in indeg.items() if d == 0]
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


main()
