import csv
import sys
from collections import defaultdict
from decimal import Decimal

reader = csv.reader(sys.stdin)
header = next(reader)
cols = {h.strip(): i for i, h in enumerate(header)}
di, ai, mi = cols['date'], cols['account'], cols['amount']

months = defaultdict(lambda: [Decimal(0), Decimal(0)])  # month -> [in, out]
balances = defaultdict(Decimal)

for row in reader:
    if not row or all(not c.strip() for c in row):
        continue
    date = row[di].strip()
    account = row[ai].strip()
    amount = Decimal(row[mi].strip())
    month = date[:7]
    if amount > 0:
        months[month][0] += amount
    elif amount < 0:
        months[month][1] += -amount
    balances[account] += amount

def fmt(d):
    return str(d.quantize(Decimal('0.01')))

for month in sorted(months):
    inc, out = months[month]
    print(f"{month} {fmt(inc)} {fmt(out)} {fmt(inc - out)}")

if balances:
    top = min(balances, key=lambda a: (-balances[a], a))
    print(f"top {top} {fmt(balances[top])}")
