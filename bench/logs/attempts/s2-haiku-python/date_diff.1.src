import sys
from datetime import date

for line in sys.stdin:
    p = line.split()
    if len(p) < 2:
        continue
    a = date.fromisoformat(p[0])
    b = date.fromisoformat(p[1])
    print(abs((b - a).days))
