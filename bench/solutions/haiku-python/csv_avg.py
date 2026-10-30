import csv
import sys
from collections import defaultdict

totals = defaultdict(float)
counts = defaultdict(int)
reader = csv.reader(sys.stdin)
header = next(reader, None)
for row in reader:
    if not row or all(not f.strip() for f in row):
        continue
    name, dept, salary = (f.strip() for f in row[:3])
    totals[dept] += float(salary)
    counts[dept] += 1

for dept in sorted(totals):
    print(f"{dept} {totals[dept] / counts[dept]:.2f}")
