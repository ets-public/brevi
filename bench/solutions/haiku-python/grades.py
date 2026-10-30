import sys
from statistics import median

names = []
scores = []
for line in sys.stdin.read().splitlines():
    parts = line.strip().rsplit(None, 1)
    if len(parts) != 2:
        continue
    names.append(parts[0])
    scores.append(int(parts[1]))

if scores:
    print(f"{median(scores):.1f}")
    top = max(scores)
    for name in sorted(n for n, s in zip(names, scores) if s == top):
        print(name)
