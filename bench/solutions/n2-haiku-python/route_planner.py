import sys
import heapq


def main():
    lines = sys.stdin.read().split("\n")
    adj = {}
    best_w = {}
    idx = 0
    while idx < len(lines):
        line = lines[idx].strip()
        idx += 1
        if line == "":
            break
        parts = line.split()
        if len(parts) < 3:
            continue
        a, b, w = parts[0], parts[1], int(parts[2])
        key = (a, b) if a < b else (b, a)
        if key in best_w and best_w[key] <= w:
            continue
        best_w[key] = w
    for (a, b), w in best_w.items():
        adj.setdefault(a, []).append((b, w))
        adj.setdefault(b, []).append((a, w))

    out = []
    for line in lines[idx:]:
        parts = line.split()
        if not parts:
            continue
        if len(parts) < 2:
            continue
        src, dst = parts[0], parts[1]
        if src not in adj:
            out.append("unknown station %s" % src)
            continue
        if dst not in adj:
            out.append("unknown station %s" % dst)
            continue
        if src == dst:
            out.append("%s (0 min)" % src)
            continue
        # Dijkstra; heap key is (cost, number of stations, station list)
        heap = [(0, 1, (src,))]
        done = {}
        result = None
        while heap:
            cost, n, path = heapq.heappop(heap)
            node = path[-1]
            if node in done:
                continue
            done[node] = True
            if node == dst:
                result = (cost, path)
                break
            for nb, w in adj[node]:
                if nb not in done:
                    heapq.heappush(heap, (cost + w, n + 1, path + (nb,)))
        if result is None:
            out.append("no route %s %s" % (src, dst))
        else:
            cost, path = result
            out.append("%s (%d min)" % (" -> ".join(path), cost))
    if out:
        sys.stdout.write("\n".join(out) + "\n")


if __name__ == "__main__":
    main()
