import sys
def tm(s):
    h, m = s.split(":"); return int(h) * 60 + int(m)
def fmt(t): return "%02d:%02d" % (t // 60, t % 60)
txt = sys.stdin.read().split("\n")
trips = {}
order = []
queries = []
sec = 0
for line in txt:
    line = line.strip()
    if line == "---": sec = 1; continue
    if not line: continue
    p = line.split()
    if sec == 0:
        st = [(p[i], tm(p[i + 1])) for i in range(1, len(p) - 1, 2)]
        trips[p[0]] = st
    else:
        queries.append(p)
nodes = []  # (time, idx, train, station)
for t, st in trips.items():
    for i, (s, tt) in enumerate(st):
        nodes.append((tt, i, t, s))
nodes.sort()
out = []
for f, to, q in queries:
    qt = tm(q)
    if f == to:
        out.append("%s %s %s via -" % (f, to, fmt(qt))); continue
    best = {}
    for (tt, i, t, s) in nodes:
        cands = []
        if s == f and tt >= qt:
            cands.append((1, (t,)))
        if i > 0 and (t, i - 1) in best:
            cands.append(best[(t, i - 1)])
        for (t2, i2), (n2, l2) in best.items():
            if trips[t2][i2][0] == s and trips[t2][i2][1] + 5 <= tt and t2 != t:
                cands.append((n2 + 1, l2 + (t,)))
        if cands:
            best[(t, i)] = min(cands)
    res = None
    for (t, i), (n, l) in best.items():
        s, tt = trips[t][i]
        if s == to:
            k = (tt, n, ">".join(l))
            if res is None or k < res: res = k
    if res is None:
        out.append("%s %s unreachable" % (f, to))
    else:
        out.append("%s %s %s via %s" % (f, to, fmt(res[0]), res[2]))
print("\n".join(out))
