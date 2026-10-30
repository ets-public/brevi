import sys
from collections import defaultdict

groups = defaultdict(set)
for word in sys.stdin.read().split():
    groups["".join(sorted(word))].add(word)

lines = [" ".join(sorted(ws)) for ws in groups.values()]
for line in sorted(lines, key=lambda l: l.split()[0]):
    print(line)
