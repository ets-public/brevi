import sys, re
from itertools import groupby

T = {}
M = []
played = set()
for n, raw in enumerate(sys.stdin.read().split('\n'), 1):
    if not raw.strip(): continue
    t = raw.split()
    m = re.match(r'^(0|[1-9][0-9]*)-(0|[1-9][0-9]*)$', t[2]) if len(t) == 3 else None
    if not m:
        print("line %d: bad line" % n); continue
    h, a = t[0], t[1]
    if h == a:
        print("line %d: same team" % n); continue
    if (h, a) in played:
        print("line %d: already played" % n); continue
    played.add((h, a))
    hg, ag = int(m.group(1)), int(m.group(2))
    M.append((h, a, hg, ag))
    for x, f, g in ((h, hg, ag), (a, ag, hg)):
        s = T.setdefault(x, [0, 0, 0, 0, 0, 0, 0])  # P W D L GF GA Pts
        s[0] += 1; s[4] += f; s[5] += g
        if f > g: s[1] += 1; s[6] += 3
        elif f == g: s[2] += 1; s[6] += 1
        else: s[3] += 1
if not T:
    print("no matches"); sys.exit()

def key(x):
    s = T[x]
    return (-s[6], -(s[4] - s[5]), -s[4])
names = sorted(T, key=key)
order = []
for _, g in groupby(names, key=key):
    g = list(g)
    if len(g) > 1:
        hp = {x: 0 for x in g}
        for h, a, hg, ag in M:
            if h in hp and a in hp:
                if hg > ag: hp[h] += 3
                elif hg == ag: hp[h] += 1; hp[a] += 1
                else: hp[a] += 3
        g.sort(key=lambda x: (-hp[x], x))
    order += g
print("Pos Team P W D L GF GA GD Pts")
for i, x in enumerate(order, 1):
    s = T[x]
    gd = s[4] - s[5]
    print("%d %s %d %d %d %d %d %d %s %d" % (i, x, s[0], s[1], s[2], s[3], s[4], s[5], ("+%d" % gd if gd > 0 else str(gd)), s[6]))
mg = max(order, key=lambda x: T[x][4])
bd = min(order, key=lambda x: T[x][5])
print("most goals: %s (%d)" % (mg, T[mg][4]))
print("best defence: %s (%d)" % (bd, T[bd][5]))
