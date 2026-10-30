import sys
tot = {}; cnt = {}
for l in sys.stdin:
    f = l.rstrip('\n').split(',')
    if len(f) < 3: continue
    tot[f[1]] = tot.get(f[1], 0) + int(f[2]); cnt[f[1]] = cnt.get(f[1], 0) + 1
for d, t in sorted(tot.items(), key=lambda p: (-p[1], p[0]))[:5]: print(d, t, cnt[d])
