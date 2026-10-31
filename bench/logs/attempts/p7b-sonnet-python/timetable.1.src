import sys
raw = sys.stdin.read().split('\n')
trips = {}
i = 0
def tm(s):
    h, m = s.split(':'); return int(h) * 60 + int(m)
def fm(x):
    return '%02d:%02d' % (x // 60, x % 60)
while i < len(raw) and raw[i].strip() != '---':
    p = raw[i].split()
    i += 1
    if len(p) < 3: continue
    st = [(p[j], tm(p[j + 1])) for j in range(1, len(p) - 1, 2)]
    trips[p[0]] = st
i += 1
at = {}
for t, st in trips.items():
    for idx, (s, _) in enumerate(st):
        at.setdefault(s, []).append((t, idx))
for line in raw[i:]:
    p = line.split()
    if len(p) != 3: continue
    fr, to, q = p[0], p[1], tm(p[2])
    if fr == to:
        print('%s %s %s via -' % (fr, to, p[2])); continue
    best = None
    layer = {}
    for t, idx in at.get(fr, []):
        if trips[t][idx][1] >= q:
            layer[(t, idx)] = [t]
    k = 1
    while layer and k <= len(trips) + 1:
        nxt = {}
        for (t, idx), lst in layer.items():
            st = trips[t]
            dep = st[idx][1]
            for j in range(idx + 1, len(st)):
                s, a = st[j]
                if s == to:
                    cand = (a, k, '>'.join(lst))
                    if best is None or cand < best: best = cand
                for t2, i2 in at.get(s, []):
                    if t2 != t and trips[t2][i2][1] >= a + 5:
                        key = (t2, i2)
                        nl = lst + [t2]
                        if key not in nxt or '>'.join(nl) < '>'.join(nxt[key]):
                            nxt[key] = nl
        layer = nxt; k += 1
    if best is None:
        print('%s %s unreachable' % (fr, to))
    else:
        print('%s %s %s via %s' % (fr, to, fm(best[0]), best[2]))
