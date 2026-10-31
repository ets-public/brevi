import sys
g = {}
for w in set(x.lower() for x in sys.stdin.read().split()):
    g.setdefault(''.join(sorted(w)), []).append(w)
groups = [sorted(v) for v in g.values() if len(v) > 1]
groups.sort(key=lambda v: (-len(v), v[0]))
for v in groups:
    print(' '.join(v))
