import sys
import csv
from decimal import Decimal, ROUND_HALF_UP

TWO = Decimal("0.01")


def fmt(d):
    return str(d.quantize(TWO, rounding=ROUND_HALF_UP))


rows = csv.reader(sys.stdin.read().splitlines())
header = next(rows, None)

months = {}
balances = {}
for row in rows:
    if len(row) < 3:
        continue
    date = row[0].strip()
    account = row[1].strip()
    amount = Decimal(row[2].strip())
    m = date[:7]
    inc, out = months.get(m, (Decimal("0"), Decimal("0")))
    if amount >= 0:
        inc += amount
    else:
        out += -amount
    months[m] = (inc, out)
    balances[account] = balances.get(account, Decimal("0")) + amount

for m in sorted(months):
    inc, out = months[m]
    print(f"{m} {fmt(inc)} {fmt(out)} {fmt(inc - out)}")

if balances:
    best = sorted(balances.items(), key=lambda kv: (-kv[1], kv[0]))[0]
    print(f"top {best[0]} {fmt(best[1])}")
