import sys, datetime
rooms = {}
bk = {}
out = []
def mins(t):
    h, m = t.split(':'); return int(h) * 60 + int(m)
for l in sys.stdin.read().split('\n'):
    p = l.split()
    if not p: continue
    c = p[0]
    if c == 'ROOM':
        rooms[p[1]] = int(p[2])
    elif c == 'BOOK':
        id_, d, s, e, n = p[1], p[2], mins(p[3]), mins(p[4]), int(p[5])
        best = None
        for name, cap in sorted(rooms.items(), key=lambda x: (x[1], x[0])):
            if cap < n: continue
            if any(b[1] == name and b[0] == d and b[2] < e and s < b[3] for b in bk.values()): continue
            best = name; break
        if best is None: out.append(id_ + ' rejected')
        else:
            bk[id_] = (d, best, s, e, p[3], p[4]); out.append('%s -> %s' % (id_, best))
    elif c == 'CANCEL':
        if p[1] in bk: del bk[p[1]]; out.append(p[1] + ' cancelled')
        else: out.append(p[1] + ' not found')
    elif c == 'LIST':
        d = p[1]
        y, mo, da = map(int, d.split('-'))
        out.append('%s %s' % (d, datetime.date(y, mo, da).strftime('%A')))
        items = sorted((b[2], b[1], i, b) for i, b in bk.items() if b[0] == d)
        if not items: out.append('  no bookings')
        for s, r, i, b in items: out.append('  %s-%s %s %s' % (b[4], b[5], r, i))
if out: print('\n'.join(out))
