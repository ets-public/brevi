import sys
g = {}
for w in sys.stdin.read().split():
    w = w.lower()
    g.setdefault("".join(sorted(w)), set()).add(w)
groups = [sorted(s) for s in g.values() if len(s) > 1]
groups.sort(key=lambda x: (-len(x), x[0]))
for x in groups:
    print(" ".join(x))
