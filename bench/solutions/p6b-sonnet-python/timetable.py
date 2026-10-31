import sys
def tm(s):
    h, m = s.split(':'); return int(h) * 60 + int(m)
def fm(x):
    return '%02d:%02d' % (x // 60, x % 60)
text = sys.stdin.read().split('\n')
trips = {}
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
        trips[p[0]] = [(p[i], tm(p[i + 1])) for i in range(1, len(p) - 1, 2)]
    else:
        queries.append(p)
states = []
bystation = {}
for t, st in trips.items():
    for k, (s, tmv) in enumerate(st):
        states.append((tmv, k, t, s))
        if k < len(st) - 1:
            bystation.setdefault(s, []).append((t, k, tmv))
states.sort()
for fr, to, qt in queries:
    qt = tm(qt)
    if fr == to:
        print(fr, to, fm(qt), 'via -'); continue
    lab = {}
    for t, k, tmv in bystation.get(fr, []):
        if tmv >= qt:
            key = (1, t)
            if (t, k) not in lab or key < lab[(t, k)]:
                lab[(t, k)] = key
    best = None
    for tmv, k, t, s in states:
        key = (t, k)
        if key not in lab:
            continue
        n, path = lab[key]
        if s == to:
            cand = (tmv, n, path)
            if best is None or cand < best:
                best = cand
        st = trips[t]
        if k + 1 < len(st):
            nk = (t, k + 1)
            if nk not in lab or (n, path) < lab[nk]:
                lab[nk] = (n, path)
        for t2, k2, d in bystation.get(s, []):
            if t2 != t and d >= tmv + 5:
                nl = (n + 1, path + '>' + t2)
                if (t2, k2) not in lab or nl < lab[(t2, k2)]:
                    lab[(t2, k2)] = nl
    if best is None:
        print(fr, to, 'unreachable')
    else:
        print(fr, to, fm(best[0]), 'via', best[2])
