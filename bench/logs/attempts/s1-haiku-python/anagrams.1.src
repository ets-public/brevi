import sys
from collections import defaultdict

words = set(w.lower() for w in sys.stdin.read().split())

groups = defaultdict(list)
for w in words:
    groups["".join(sorted(w))].append(w)

result = [sorted(g) for g in groups.values() if len(g) > 1]
result.sort(key=lambda g: (-len(g), g[0]))
for g in result:
    print(" ".join(g))
