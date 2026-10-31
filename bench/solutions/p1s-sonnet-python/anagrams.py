import sys
g = {}
for wd in sys.stdin.read().split():
    wd = wd.lower()
    g.setdefault(''.join(sorted(wd)), set()).add(wd)
groups = [sorted(s) for s in g.values() if len(s) > 1]
groups.sort(key=lambda s: (-len(s), s[0]))
for s in groups:
    print(' '.join(s))
