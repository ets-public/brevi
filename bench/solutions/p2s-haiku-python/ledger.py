import sys
import csv
from decimal import Decimal


def fmt(cents):
    sign = "-" if cents < 0 else ""
    c = abs(cents)
    return "%s%d.%02d" % (sign, c // 100, c % 100)


def to_cents(s):
    return int((Decimal(s.strip()) * 100).to_integral_value())


reader = csv.reader(sys.stdin)
next(reader, None)

months = {}
balances = {}
for row in reader:
    if len(row) < 3:
        continue
    date, account, amt = row[0].strip(), row[1].strip(), row[2]
    month = date[:7]
    cents = to_cents(amt)
    m = months.setdefault(month, [0, 0])
    if cents > 0:
        m[0] += cents
    elif cents < 0:
        m[1] += -cents
    balances[account] = balances.get(account, 0) + cents

for month in sorted(months):
    inn, out = months[month]
    print("%s %s %s %s" % (month, fmt(inn), fmt(out), fmt(inn - out)))

if balances:
    top = min(balances.items(), key=lambda kv: (-kv[1], kv[0]))
    print("top %s %s" % (top[0], fmt(top[1])))
