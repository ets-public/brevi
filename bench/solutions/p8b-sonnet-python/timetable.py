import sys

def pt(s):
    h, m = s.split(":")
    return int(h) * 60 + int(m)

def ft(x):
    return "%02d:%02d" % (x // 60, x % 60)

text = sys.stdin.read().split("\n")
trips = []
queries = []
sec = 0
for line in text:
    line = line.strip()
    if line == "---":
        sec = 1
        continue
    if not line:
        continue
    p = line.split()
    if sec == 0:
        stops = []
        for k in range(1, len(p) - 1, 2):
            stops.append((p[k], pt(p[k + 1])))
        trips.append((p[0], stops))
    else:
        queries.append((p[0], p[1], pt(p[2])))

nodes = []
for ti, (name, stops) in enumerate(trips):
    for si, (st, t) in enumerate(stops):
        nodes.append((t, si, ti))
nodes.sort()

out = []
for frm, to, qt in queries:
    if frm == to:
        out.append("%s %s %s via -" % (frm, to, ft(qt)))
        continue
    best = {}
    for t, si, ti in nodes:
        name, stops = trips[ti]
        st = stops[si][0]
        cands = []
        if st == frm and t >= qt and si < len(stops) - 1:
            cands.append((1, name, [name]))
        if si > 0 and (ti, si - 1) in best:
            cands.append(best[(ti, si - 1)])
        if si < len(stops) - 1:
            for (uj, sj), key in best.items():
                if uj == ti:
                    continue
                uname, ustops = trips[uj]
                if ustops[sj][0] == st and ustops[sj][1] + 5 <= t and sj >= 1:
                    cands.append((key[0] + 1, None, key[2] + [name]))
        if cands:
            bk = None
            for c in cands:
                k = (c[0], ">".join(c[2]))
                if bk is None or k < bk[0]:
                    bk = (k, c)
            best[(ti, si)] = (bk[1][0], bk[0][1], bk[1][2])
    res = None
    for (ti, si), key in best.items():
        name, stops = trips[ti]
        if si >= 1 and stops[si][0] == to:
            k = (stops[si][1], key[0], key[1])
            if res is None or k < res:
                res = k
    if res is None:
        out.append("%s %s unreachable" % (frm, to))
    else:
        out.append("%s %s %s via %s" % (frm, to, ft(res[0]), res[2]))
print("\n".join(out))
