import sys
import csv
from decimal import Decimal

reader = csv.reader(sys.stdin)
header = next(reader, None)

months = {}
balances = {}
for row in reader:
    if len(row) < 3:
        continue
    date, account, amt = row[0].strip(), row[1].strip(), Decimal(row[2].strip())
    m = date[:7]
    inn, out = months.get(m, (Decimal("0"), Decimal("0")))
    if amt > 0:
        inn += amt
    elif amt < 0:
        out += -amt
    months[m] = (inn, out)
    balances[account] = balances.get(account, Decimal("0")) + amt

for m in sorted(months):
    inn, out = months[m]
    print(f"{m} {inn:.2f} {out:.2f} {(inn - out):.2f}")

top = min(balances.items(), key=lambda kv: (-kv[1], kv[0]))
print(f"top {top[0]} {top[1]:.2f}")
