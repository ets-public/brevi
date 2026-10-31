import sys
from datetime import date

for line in sys.stdin:
    parts = line.split()
    if len(parts) < 2:
        continue
    a = date.fromisoformat(parts[0])
    b = date.fromisoformat(parts[1])
    print(abs((a - b).days))
