# Reading a missing key of a defaultdict puts it in: len, loops and printing see it.
from collections import defaultdict

n = defaultdict(int)
n["x"] += 2
print(n["x"], n["y"], len(n), sorted(n))
adj = defaultdict(list)
for a, b in [(1, 2), (2, 3)]:
    adj[a].append(b)
print([v for v in adj[9]], len(adj), dict(adj))
