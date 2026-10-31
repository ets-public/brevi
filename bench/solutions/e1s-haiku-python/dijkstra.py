import sys
import heapq


def main():
    lines = [l.split() for l in sys.stdin.read().splitlines() if l.strip()]
    if not lines:
        return
    src, dst = lines[-1][0], lines[-1][1]
    adj = {}
    for p in lines[:-1]:
        if len(p) < 3:
            continue
        a, b, w = p[0], p[1], int(p[2])
        if a == b:
            continue
        if b not in adj.get(a, {}) or w < adj[a][b]:
            adj.setdefault(a, {})[b] = w
        if a not in adj.get(b, {}) or w < adj[b][a]:
            adj.setdefault(b, {})[a] = w

    # distances to dst (graph is undirected)
    INF = float("inf")
    dist = {}
    if dst in adj or src == dst:
        dist[dst] = 0
        pq = [(0, dst)]
        while pq:
            d, u = heapq.heappop(pq)
            if d > dist.get(u, INF):
                continue
            for v, w in adj.get(u, {}).items():
                nd = d + w
                if nd < dist.get(v, INF):
                    dist[v] = nd
                    heapq.heappush(pq, (nd, v))

    if src not in dist:
        print("none")
        return

    total = dist[src]
    path = [src]
    cur = src
    while cur != dst:
        best = None
        for v, w in adj.get(cur, {}).items():
            if v in dist and w + dist[v] == dist[cur]:
                if best is None or v < best:
                    best = v
        if best is None:
            break
        path.append(best)
        cur = best

    print(total)
    print("->".join(path))


main()
