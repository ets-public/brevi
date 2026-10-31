import sys

text = sys.stdin.read().split("\n")
sec = 0
trains = {}
queries = []


def tm(s):
    h, m = s.split(":")
    return int(h) * 60 + int(m)


def fmt(t):
    return "%02d:%02d" % (t // 60, t % 60)


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
        for i in range(1, len(p) - 1, 2):
            stops.append((p[i], tm(p[i + 1])))
        trains[p[0]] = stops
    else:
        queries.append((p[0], p[1], tm(p[2])))

INF = None
for fr, to, t in queries:
    if fr == to:
        print("%s %s %s via -" % (fr, to, fmt(t)))
        continue
    val = {}
    for name, stops in trains.items():
        for j, (st, tt) in enumerate(stops):
            if st == fr and tt >= t:
                val[(name, j)] = (1, (name,))
    changed = True
    while changed:
        changed = False
        # propagate along trains
        for name, stops in trains.items():
            cur = None
            for j in range(len(stops)):
                v = val.get((name, j))
                if cur is not None and (v is None or cur < v):
                    val[(name, j)] = cur
                    v = cur
                    changed = True
                if v is not None and (cur is None or v < cur):
                    cur = v
        # transfers
        for name, stops in trains.items():
            for j in range(1, len(stops)):
                v = val.get((name, j))
                if v is None:
                    continue
                st, a = stops[j]
                for n2, s2 in trains.items():
                    if n2 == name:
                        continue
                    for k, (st2, d2) in enumerate(s2):
                        if st2 == st and d2 >= a + 5 and k < len(s2) - 1:
                            nv = (v[0] + 1, v[1] + (n2,))
                            o = val.get((n2, k))
                            if o is None or nv < o:
                                val[(n2, k)] = nv
                                changed = True
    best = None
    for (name, j), v in val.items():
        if j >= 1 and trains[name][j][0] == to:
            key = (trains[name][j][1], v[0], v[1])
            if best is None or key < best:
                best = key
    if best is None:
        print("%s %s unreachable" % (fr, to))
    else:
        print("%s %s %s via %s" % (fr, to, fmt(best[0]), ">".join(best[2])))
