import sys
import csv
from decimal import Decimal


def to_cents(s):
    d = Decimal(s.strip())
    return int((d * 100).to_integral_value())


def fmt(cents):
    sign = "-" if cents < 0 else ""
    c = abs(cents)
    return "%s%d.%02d" % (sign, c // 100, c % 100)


def main():
    text = sys.stdin.read()
    reader = csv.reader(text.splitlines())
    totals = {}
    header_skipped = False
    for row in reader:
        if not row or all(not f.strip() for f in row):
            continue
        if not header_skipped:
            header_skipped = True
            if row[0].strip().lower() == "item":
                continue
        if len(row) < 3:
            continue
        item = row[0].strip()
        qty = int(row[1].strip())
        cents = to_cents(row[2])
        totals[item] = totals.get(item, 0) + qty * cents
    items = sorted(totals.items(), key=lambda kv: (-kv[1], kv[0]))
    for name, val in items:
        print("%s: %s" % (name, fmt(val)))


main()
