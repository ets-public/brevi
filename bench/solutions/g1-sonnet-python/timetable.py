import sys

def tm(s):
    h, m = s.split(':')
    return int(h) * 60 + int(m)
def ft(x):
    return '%02d:%02d' % (x // 60, x % 60)

text = sys.stdin.read().split('\n')
trains = {}
queries = []
sec = 0
for l in text:
    l = l.strip()
    if l == '---':
        sec = 1; continue
    if not l:
        continue
    p = l.split()
    if sec == 0:
        stops = [(p[i], tm(p[i + 1])) for i in range(1, len(p) - 1, 2)]
        trains[p[0]] = stops
    else:
        queries.append(p)

states = []  # (time, idx, train)
for t, st in trains.items():
    for i, (s, tt) in enumerate(st):
        states.append((tt, i, t))
states.sort()
bystation = {}
for tt, i, t in states:
    bystation.setdefault(trains[t][i][0], []).append((tt, i, t))

def key(v):
    return (v[0], '>'.join(v[1]))

out = []
for q in queries:
    F, T, qt = q[0], q[1], tm(q[2])
    if F == T:
        out.append('%s %s %s via -' % (F, T, q[2]))
        continue
    best = {}
    for tt, i, t in bystation.get(F, []):
        if tt >= qt:
            c = (1, (t,))
            if (t, i) not in best or key(c) < key(best[(t, i)]):
                best[(t, i)] = c
    ans = None
    for tt, i, t in states:
        v = best.get((t, i))
        if v is None:
            continue
        st = trains[t]
        s = st[i][0]
        if s == T:
            cand = (tt, v[0], '>'.join(v[1]), v[1])
            if ans is None or cand[:3] < ans[:3]:
                ans = cand
        if i + 1 < len(st):
            k = (t, i + 1)
            if k not in best or key(v) < key(best[k]):
                best[k] = v
        for tt2, k2, u in bystation.get(s, []):
            if u != t and tt2 >= tt + 5:
                c = (v[0] + 1, v[1] + (u,))
                if (u, k2) not in best or key(c) < key(best[(u, k2)]):
                    best[(u, k2)] = c
    if ans is None:
        out.append('%s %s unreachable' % (F, T))
    else:
        out.append('%s %s %s via %s' % (F, T, ft(ans[0]), ans[2]))
print('\n'.join(out))
