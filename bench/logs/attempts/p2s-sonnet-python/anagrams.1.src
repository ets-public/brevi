import sys
words = set(sys.stdin.read().lower().split())
g = {}
for wd in words:
    g.setdefault(''.join(sorted(wd)), []).append(wd)
groups = [sorted(v) for v in g.values() if len(v) > 1]
groups.sort(key=lambda v: (-len(v), v[0]))
for v in groups:
    print(' '.join(v))
