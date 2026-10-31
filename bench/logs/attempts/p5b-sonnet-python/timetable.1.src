import sys
def tm(s):
    h, m = s.split(":"); return int(h) * 60 + int(m)
def fmt(x):
    return "%02d:%02d" % (x // 60, x % 60)
text = sys.stdin.read().split("\n")
trains = []
qs = []
sec = 0
for line in text:
    line = line.strip()
    if line == "---":
        sec = 1; continue
    if not line: continue
    p = line.split()
    if sec == 0:
        stops = [(p[i], tm(p[i + 1])) for i in range(1, len(p) - 1, 2)]
        trains.append((p[0], stops))
    else:
        qs.append(p)
for f, t, q in qs:
    q = tm(q)
    if f == t:
        print("%s %s %s via -" % (f, t, fmt(q))); continue
    cur = {f: (q, [])}
    best = None
    for k in range(1, len(trains) + 1):
        nxt = {}
        for name, stops in trains:
            for i, (st, dep) in enumerate(stops):
                if st not in cur: continue
                arr, lst = cur[st]
                if k > 1: arr += 5
                if dep < arr: continue
                nl = lst + [name]
                for st2, a2 in stops[i + 1:]:
                    c = nxt.get(st2)
                    if c is None or (a2, ">".join(nl)) < (c[0], ">".join(c[1])):
                        nxt[st2] = (a2, nl)
                break
        if t in nxt:
            c = nxt[t]
            if best is None or c[0] < best[0]:
                best = c
        cur = nxt
        if not cur: break
    if best is None:
        print("%s %s unreachable" % (f, t))
    else:
        print("%s %s %s via %s" % (f, t, fmt(best[0]), ">".join(best[1])))
