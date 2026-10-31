import sys, heapq
lines = sys.stdin.read().split('\n')
g = {}
i = 0
while i < len(lines) and lines[i].strip() != '':
    p = lines[i].split(); i += 1
    if len(p) < 3: continue
    a, b, m = p[0], p[1], int(p[2])
    g.setdefault(a, {}); g.setdefault(b, {})
    if b not in g[a] or m < g[a][b]:
        g[a][b] = m; g[b][a] = m
out = []
for l in lines[i:]:
    p = l.split()
    if not p: continue
    s, t = p[0], p[1]
    if s not in g: out.append("unknown station " + s); continue
    if t not in g: out.append("unknown station " + t); continue
    if s == t: out.append("%s (0 min)" % s); continue
    h = [(0, 1, (s,))]
    done = set(); res = None
    while h:
        d, n, path = heapq.heappop(h)
        u = path[-1]
        if u in done: continue
        done.add(u)
        if u == t: res = (d, path); break
        for v, w in g[u].items():
            if v not in done:
                heapq.heappush(h, (d + w, n + 1, path + (v,)))
    if res is None: out.append("no route %s %s" % (s, t))
    else: out.append(" -> ".join(res[1]) + " (%d min)" % res[0])
print("\n".join(out))
