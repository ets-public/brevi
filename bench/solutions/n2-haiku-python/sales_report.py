import sys
import csv
import re
import datetime
from decimal import Decimal

DATE_RE = re.compile(r"[0-9]{4}-[0-9]{2}-[0-9]{2}")
QTY_RE = re.compile(r"[0-9]+")
PRICE_RE = re.compile(r"[0-9]+(\.[0-9]{1,2})?")


def fmt(d):
    return str(Decimal(d).quantize(Decimal("0.01")))


def main():
    data = sys.stdin.read()
    lines = [l for l in data.splitlines() if l.strip() != ""]
    rows = list(csv.reader(lines))
    if not rows:
        print("rows 0")
        print("skipped 0")
        return
    body = rows[1:]

    valid = []
    skipped = 0
    for f in body:
        if len(f) != 5:
            skipped += 1
            continue
        date, region, product, qty, price = f
        ok = True
        if not DATE_RE.fullmatch(date):
            ok = False
        else:
            try:
                datetime.datetime.strptime(date, "%Y-%m-%d")
            except ValueError:
                ok = False
        if ok and not QTY_RE.fullmatch(qty):
            ok = False
        if ok and not PRICE_RE.fullmatch(price):
            ok = False
        if not ok:
            skipped += 1
            continue
        valid.append((date, region, product, int(qty), Decimal(price)))

    print("rows %d" % len(valid))
    print("skipped %d" % skipped)
    if not valid:
        return

    regions = {}
    products = {}
    months = {}
    for date, region, product, qty, price in valid:
        rev = qty * price
        regions[region] = regions.get(region, Decimal(0)) + rev
        products[product] = products.get(product, 0) + qty
        m = date[:7]
        months[m] = months.get(m, Decimal(0)) + rev

    print("regions")
    for region, total in sorted(regions.items(), key=lambda kv: (-kv[1], kv[0])):
        print("  %s %s" % (region, fmt(total)))

    top_name, top_qty = min(products.items(), key=lambda kv: (-kv[1], kv[0]))
    print("top_product %s %d" % (top_name, top_qty))

    print("months")
    for m in sorted(months):
        print("  %s %s" % (m, fmt(months[m])))


if __name__ == "__main__":
    main()
