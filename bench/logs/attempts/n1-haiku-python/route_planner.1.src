import heapq
import sys


def main():
    lines = sys.stdin.read().split("\n")
    idx = 0
    graph = {}
    while idx < len(lines):
        line = lines[idx].strip()
        idx += 1
        if line == "":
            break
        parts = line.split()
        if len(parts) != 3:
            continue
        a, b, m = parts[0], parts[1], int(parts[2])
        if a == b:
            graph.setdefault(a, {})
            continue
        for x, y in ((a, b), (b, a)):
            adj = graph.setdefault(x, {})
            if y not in adj or m < adj[y]:
                adj[y] = m

    cache = {}

    def search(src):
        # Dijkstra over full paths; the key (minutes, stations, path list)
        # gives the required tie-breaking order.
        best = {}
        heap = [(0, 1, [src])]
        while heap:
            minutes, hops, path = heapq.heappop(heap)
            node = path[-1]
            if node in best:
                continue
            best[node] = (minutes, hops, path)
            for nxt, w in graph.get(node, {}).items():
                if nxt in best:
                    continue
                heapq.heappush(heap, (minutes + w, hops + 1, path + [nxt]))
        return best

    out = []
    for line in lines[idx:]:
        parts = line.split()
        if not parts:
            continue
        if len(parts) < 2:
            continue
        src, dst = parts[0], parts[1]
        if src not in graph:
            out.append("unknown station %s" % src)
            continue
        if dst not in graph:
            out.append("unknown station %s" % dst)
            continue
        if src == dst:
            out.append("%s (0 min)" % src)
            continue
        if src not in cache:
            cache[src] = search(src)
        res = cache[src].get(dst)
        if res is None:
            out.append("no route %s %s" % (src, dst))
        else:
            minutes, _, path = res
            out.append("%s (%d min)" % (" -> ".join(path), minutes))
    sys.stdout.write("\n".join(out) + ("\n" if out else ""))


if __name__ == "__main__":
    main()
