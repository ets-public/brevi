import sys

words = set(w.lower() for w in sys.stdin.read().split())
groups = {}
for w in words:
    groups.setdefault("".join(sorted(w)), []).append(w)

result = []
for g in groups.values():
    if len(g) > 1:
        result.append(sorted(g))

result.sort(key=lambda g: (-len(g), g[0]))
for g in result:
    print(" ".join(g))
