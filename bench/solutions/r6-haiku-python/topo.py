import sys
import heapq
from collections import defaultdict


def main():
    adj = defaultdict(set)
    indeg = {}
    for line in sys.stdin:
        line = line.strip()
        if not line:
            continue
        a, b = [x.strip() for x in line.split("->")]
        for n in (a, b):
            if n not in indeg:
                indeg[n] = 0
        if b not in adj[a]:
            adj[a].add(b)
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

    if len(order) != len(indeg):
        print("cycle")
    else:
        print("\n".join(order))


main()
