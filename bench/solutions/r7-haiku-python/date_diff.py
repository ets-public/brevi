import sys
from datetime import date

for line in sys.stdin:
    parts = line.split()
    if len(parts) < 2:
        continue
    d1 = date(*map(int, parts[0].split('-')))
    d2 = date(*map(int, parts[1].split('-')))
    print(abs((d2 - d1).days))
