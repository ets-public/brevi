import sys
import re
from datetime import date

from stock import Stock
from orders import Orders

DATE_RE = re.compile(r"^\d{4}-\d{2}-\d{2}$")
QTY_RE = re.compile(r"^\d+$")


def valid_date(s):
    if not DATE_RE.match(s):
        return False
    try:
        y, m, d = map(int, s.split("-"))
        date(y, m, d)
        return True
    except ValueError:
        return False


def positive_qty(s):
    if not QTY_RE.match(s):
        return None
    q = int(s)
    return q if q > 0 else None


def main():
    lines = sys.stdin.read().split("\n")
    today = None
    stock = Stock()
    orders = Orders(stock)
    shipped_total = 0

    for idx, raw in enumerate(lines):
        lineno = idx + 1
        parts = raw.split()
        if not parts:
            continue
        kw = parts[0]

        if kw == "DAY":
            if len(parts) != 2 or not valid_date(parts[1]):
                print("line %d: bad line" % lineno)
                continue
            new_day = parts[1]
            if today is not None and new_day < today:
                print("line %d: date goes back" % lineno)
                continue
            today = new_day
            for sku, qty, expiry in stock.discard_before(today):
                print("discard %s %d %s" % (sku, qty, expiry))

        elif kw == "RECV":
            if len(parts) != 4 or positive_qty(parts[2]) is None or not valid_date(parts[3]):
                print("line %d: bad line" % lineno)
                continue
            if today is None:
                print("line %d: no date" % lineno)
                continue
            sku, qty, expiry = parts[1], positive_qty(parts[2]), parts[3]
            if expiry < today:
                print("line %d: expired on arrival" % lineno)
                continue
            stock.add(sku, qty, expiry)
            got, msgs = orders.fill(sku)
            shipped_total += got
            for m in msgs:
                print(m)

        elif kw == "ORDER":
            if len(parts) != 4 or positive_qty(parts[3]) is None:
                print("line %d: bad line" % lineno)
                continue
            if today is None:
                print("line %d: no date" % lineno)
                continue
            oid, sku, qty = parts[1], parts[2], positive_qty(parts[3])
            if orders.has(oid):
                print("line %d: duplicate order" % lineno)
                continue
            shipped, msgs = orders.place(oid, sku, qty)
            shipped_total += shipped
            for m in msgs:
                print(m)

        elif kw == "STOCK":
            if len(parts) != 1:
                print("line %d: bad line" % lineno)
                continue
            rows = stock.summary()
            if not rows:
                print("stock empty")
            for sku, units, count in rows:
                print("%s %d in %d lots" % (sku, units, count))

        else:
            print("line %d: bad line" % lineno)

    print("shipped %d units, backordered %d units" % (shipped_total, orders.waiting()))


if __name__ == "__main__":
    main()
