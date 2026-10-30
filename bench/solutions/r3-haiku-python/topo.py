import sys
import heapq
from collections import defaultdict


def main():
    adj = defaultdict(set)
    indeg = defaultdict(int)
    nodes = set()
    for line in sys.stdin.read().split("\n"):
        if "->" not in line:
            continue
        a, b = line.split("->", 1)
        a = a.strip()
        b = b.strip()
        if not a or not b:
            continue
        nodes.add(a)
        nodes.add(b)
        if b not in adj[a]:
            adj[a].add(b)
            indeg[b] += 1
    for n in nodes:
        indeg.setdefault(n, 0)

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


main()
