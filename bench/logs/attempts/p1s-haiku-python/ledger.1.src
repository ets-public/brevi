import sys
import csv
from decimal import Decimal


def fmt(c):
    sign = "-" if c < 0 else ""
    c = abs(c)
    return "%s%d.%02d" % (sign, c // 100, c % 100)


def to_cents(s):
    return int((Decimal(s.strip()) * 100).to_integral_value())


rows = csv.reader(sys.stdin)
next(rows, None)

months = {}
accounts = {}
for r in rows:
    if len(r) < 3:
        continue
    date, account, amount = r[0].strip(), r[1].strip(), r[2]
    cents = to_cents(amount)
    m = date[:7]
    inn, out = months.get(m, (0, 0))
    if cents > 0:
        inn += cents
    elif cents < 0:
        out += -cents
    months[m] = (inn, out)
    accounts[account] = accounts.get(account, 0) + cents

for m in sorted(months):
    inn, out = months[m]
    print("%s %s %s %s" % (m, fmt(inn), fmt(out), fmt(inn - out)))

if accounts:
    top = sorted(accounts.items(), key=lambda kv: (-kv[1], kv[0]))[0]
    print("top %s %s" % (top[0], fmt(top[1])))
