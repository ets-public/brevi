import sys
import heapq
from collections import defaultdict


def dijkstra(adj, src):
    dist = {src: 0}
    heap = [(0, src)]
    while heap:
        d, u = heapq.heappop(heap)
        if d > dist.get(u, float("inf")):
            continue
        for v, w in adj[u]:
            nd = d + w
            if nd < dist.get(v, float("inf")):
                dist[v] = nd
                heapq.heappush(heap, (nd, v))
    return dist


def main():
    lines = [l.split() for l in sys.stdin.read().split("\n") if l.strip()]
    if not lines:
        return
    frm, to = lines[-1][0], lines[-1][1]
    adj = defaultdict(list)
    for parts in lines[:-1]:
        if len(parts) < 3:
            continue
        a, b, w = parts[0], parts[1], int(parts[2])
        adj[a].append((b, w))
        adj[b].append((a, w))

    ds = dijkstra(adj, frm)
    if to not in ds:
        print("none")
        return
    dt = dijkstra(adj, to)
    total = ds[to]

    path = [frm]
    cur = frm
    while cur != to:
        best = None
        for v, w in adj[cur]:
            if v in ds and v in dt and ds[cur] + w + dt[v] == total and ds[v] == ds[cur] + w:
                if best is None or v < best:
                    best = v
        if best is None:
            break
        path.append(best)
        cur = best

    print(total)
    print("->".join(path))


main()
