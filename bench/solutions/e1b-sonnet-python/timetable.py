import sys
def tm(s):
    h, m = s.split(':'); return int(h) * 60 + int(m)
def fmt(x):
    return '%02d:%02d' % (x // 60, x % 60)
data = sys.stdin.read().split('\n')
trains = {}
sec = 0; queries = []
for line in data:
    line = line.strip()
    if line == '---':
        sec = 1; continue
    if not line: continue
    p = line.split()
    if sec == 0:
        trains[p[0]] = [(p[i], tm(p[i + 1])) for i in range(1, len(p) - 1, 2)]
    else:
        queries.append(p)
at = {}
for t, st in trains.items():
    for i, (s, tt) in enumerate(st[:-1]):
        at.setdefault(s, []).append((t, i))
out = []
for f, to, q in queries:
    qt = tm(q)
    if f == to:
        out.append('%s %s %s via -' % (f, to, q)); continue
    best = None
    layer = {f: {qt - 5: ()}}
    for rnd in range(1, 12):
        new = {}
        for s, d in layer.items():
            for arr, tup in d.items():
                minDep = qt if rnd == 1 else arr + 5
                for t, i in at.get(s, []):
                    st = trains[t]
                    if st[i][1] < minDep: continue
                    for j in range(i + 1, len(st)):
                        s2, a2 = st[j]
                        nt = tup + (t,)
                        dd = new.setdefault(s2, {})
                        if a2 not in dd or nt < dd[a2]:
                            dd[a2] = nt
        if to in new:
            for a, tup in new[to].items():
                cand = (a, rnd, '>'.join(tup))
                if best is None or cand < best: best = cand
        if not new: break
        layer = new
    if best is None:
        out.append('%s %s unreachable' % (f, to))
    else:
        out.append('%s %s %s via %s' % (f, to, fmt(best[0]), best[2]))
print('\n'.join(out))
