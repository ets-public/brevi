import sys

text = sys.stdin.read().split("\n")
sep = text.index("---") if "---" in text else len(text)
trips = {}
for l in text[:sep]:
    p = l.split()
    if len(p) < 3:
        continue
    st = []
    for k in range(1, len(p) - 1, 2):
        h, m = p[k + 1].split(":")
        st.append((p[k], int(h) * 60 + int(m)))
    trips[p[0]] = st
queries = [l.split() for l in text[sep + 1:] if l.strip()]


def fmt(t):
    return "%02d:%02d" % (t // 60, t % 60)


states = []
for tr, st in trips.items():
    for i, (s, t) in enumerate(st):
        states.append((t, i, tr, s))
states.sort(key=lambda x: (x[0], x[1], x[2]))

for q in queries:
    if len(q) < 3:
        continue
    fr, to, qt = q[0], q[1], q[2]
    h, m = qt.split(":")
    qt = int(h) * 60 + int(m)
    if fr == to:
        print("%s %s %s via -" % (fr, to, fmt(qt)))
        continue
    best = {}
    done = []
    for t, i, tr, s in states:
        cands = []
        if s == fr and t >= qt:
            cands.append((1, [tr]))
        if i > 0 and (tr, i - 1) in best:
            n, l = best[(tr, i - 1)]
            cands.append((n, l))
        for (t2, i2, tr2, s2) in done:
            if s2 == s and tr2 != tr and t2 + 5 <= t and (tr2, i2) in best:
                n, l = best[(tr2, i2)]
                cands.append((n + 1, l + [tr]))
        if cands:
            best[(tr, i)] = min(cands, key=lambda c: (c[0], ">".join(c[1])))
        done.append((t, i, tr, s))
    res = None
    for t, i, tr, s in states:
        if s == to and (tr, i) in best and i > 0:
            n, l = best[(tr, i)]
            key = (t, n, ">".join(l))
            if res is None or key < res:
                res = key
    if res is None:
        print("%s %s unreachable" % (fr, to))
    else:
        print("%s %s %s via %s" % (fr, to, fmt(res[0]), res[2]))
