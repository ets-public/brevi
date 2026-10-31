import sys

def tm(s):
    h, m = s.split(':')
    return int(h) * 60 + int(m)

def ft(x):
    return "%02d:%02d" % (x // 60, x % 60)

lines = [l.strip() for l in sys.stdin.read().split('\n')]
if '---' in lines:
    k = lines.index('---')
    ttl, ql = lines[:k], lines[k + 1:]
else:
    ttl, ql = lines, []
trips = []
for l in ttl:
    p = l.split()
    if len(p) < 3:
        continue
    stops = [(p[i], tm(p[i + 1])) for i in range(1, len(p) - 1, 2)]
    trips.append((p[0], stops))

events = []  # (time, tripidx, j)
for ti, (name, stops) in enumerate(trips):
    for j, (st, t) in enumerate(stops):
        events.append((t, ti, j))
events.sort()

for q in ql:
    p = q.split()
    if len(p) < 3:
        continue
    fr, to, qt = p[0], p[1], tm(p[2])
    if fr == to:
        print("%s %s %s via -" % (fr, to, ft(qt)))
        continue
    aboard = {}   # (ti, j) -> tuple of names
    rides = []    # (station, arrival time, names tuple) from ride arrivals
    best = None
    def key(names):
        return (len(names), ">".join(names))
    for t, ti, j in events:
        name, stops = trips[ti]
        st = stops[j][0]
        # ride arrival at j from aboard (ti, j-1)
        if j >= 1 and (ti, j - 1) in aboard:
            names = aboard[(ti, j - 1)]
            aboard[(ti, j)] = names if (ti, j) not in aboard or key(names) < key(aboard[(ti, j)]) else aboard[(ti, j)]
            rides.append((st, t, names))
            if st == to:
                cand = (t, key(names), names)
                if best is None or cand[:2] < best[:2]:
                    best = cand
        # boarding at j
        if j < len(stops) - 1:
            cands = []
            if st == fr and t >= qt:
                cands.append((name,))
            for rs, ra, rn in rides:
                if rs == st and ra + 5 <= t:
                    cands.append(rn + (name,))
            for c in cands:
                if (ti, j) not in aboard or key(c) < key(aboard[(ti, j)]):
                    aboard[(ti, j)] = c
    if best is None:
        print("%s %s unreachable" % (fr, to))
    else:
        print("%s %s %s via %s" % (fr, to, ft(best[0]), ">".join(best[2])))
