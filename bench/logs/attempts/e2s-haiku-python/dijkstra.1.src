import heapq
import sys


def main():
    lines = [l.split() for l in sys.stdin.read().splitlines() if l.strip()]
    if not lines:
        return
    query = lines[-1]
    edges = lines[:-1]
    adj = {}
    for parts in edges:
        if len(parts) < 3:
            continue
        a, b, w = parts[0], parts[1], int(parts[2])
        adj.setdefault(a, []).append((b, w))
        adj.setdefault(b, []).append((a, w))
    src, dst = query[0], query[1]

    # distances from every node to dst (graph is undirected)
    INF = float("inf")
    dist = {dst: 0}
    pq = [(0, dst)]
    while pq:
        d, u = heapq.heappop(pq)
        if d > dist.get(u, INF):
            continue
        for v, w in adj.get(u, []):
            nd = d + w
            if nd < dist.get(v, INF):
                dist[v] = nd
                heapq.heappush(pq, (nd, v))

    if src not in dist:
        print("none")
        return

    path = [src]
    cur = src
    while cur != dst:
        best = None
        for v, w in adj.get(cur, []):
            if v in dist and dist[v] + w == dist[cur]:
                if best is None or v < best:
                    best = v
        if best is None:
            break
        path.append(best)
        cur = best

    print(dist[src])
    print("->".join(path))


main()
