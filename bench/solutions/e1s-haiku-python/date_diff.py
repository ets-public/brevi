import sys
from datetime import date

for line in sys.stdin:
    parts = line.split()
    if len(parts) < 2:
        continue
    y1, m1, d1 = map(int, parts[0].split("-"))
    y2, m2, d2 = map(int, parts[1].split("-"))
    a = date(y1, m1, d1).toordinal()
    b = date(y2, m2, d2).toordinal()
    print(abs(a - b))
