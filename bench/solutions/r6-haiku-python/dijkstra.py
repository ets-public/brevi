import sys
import heapq
from collections import defaultdict


def main():
    lines = [l.split() for l in sys.stdin if l.strip()]
    query = lines[-1]
    src, dst = query[0], query[1]
    adj = defaultdict(list)
    for p in lines[:-1]:
        a, b, w = p[0], p[1], int(p[2])
        adj[a].append((b, w))
        adj[b].append((a, w))

    if src not in adj and src != dst:
        print("none")
        return

    # distances to dst (graph is undirected)
    INF = float("inf")
    dist = defaultdict(lambda: INF)
    dist[dst] = 0
    heap = [(0, dst)]
    while heap:
        d, u = heapq.heappop(heap)
        if d > dist[u]:
            continue
        for v, w in adj[u]:
            nd = d + w
            if nd < dist[v]:
                dist[v] = nd
                heapq.heappush(heap, (nd, v))

    if dist[src] == INF:
        print("none")
        return

    path = [src]
    cur = src
    while cur != dst:
        best = None
        for v, w in adj[cur]:
            if dist[v] != INF and w + dist[v] == dist[cur]:
                if best is None or v < best:
                    best = v
        cur = best
        path.append(cur)

    print(dist[src])
    print("->".join(path))


main()
