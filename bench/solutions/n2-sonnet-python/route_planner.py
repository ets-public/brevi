import sys, heapq

lines = sys.stdin.read().split("\n")
adj = {}
i = 0
while i < len(lines) and lines[i].strip() != "":
    p = lines[i].split()
    i += 1
    if len(p) < 3:
        continue
    a, b, w = p[0], p[1], int(p[2])
    adj.setdefault(a, {})
    adj.setdefault(b, {})
    if a == b:
        continue
    if b not in adj[a] or w < adj[a][b]:
        adj[a][b] = w
        adj[b][a] = w

def route(s, t):
    best = {}
    h = [(0, 1, (s,))]
    while h:
        d, n, path = heapq.heappop(h)
        u = path[-1]
        if u in best:
            continue
        best[u] = (d, n, path)
        if u == t:
            return best[u]
        for v, w in adj[u].items():
            if v not in best:
                heapq.heappush(h, (d + w, n + 1, path + (v,)))
    return None

for line in lines[i:]:
    p = line.split()
    if not p:
        continue
    f, t = p[0], p[1]
    if f not in adj:
        print("unknown station " + f)
    elif t not in adj:
        print("unknown station " + t)
    elif f == t:
        print("%s (0 min)" % f)
    else:
        r = route(f, t)
        if r is None:
            print("no route %s %s" % (f, t))
        else:
            print("%s (%d min)" % (" -> ".join(r[2]), r[0]))
