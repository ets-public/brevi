import re
import sys
import datetime

from stock import Stock
from orders import Orders

DATE_RE = re.compile(r'[0-9]{4}-[0-9]{2}-[0-9]{2}')
QTY_RE = re.compile(r'[0-9]+')


def parse_date(s):
    if not DATE_RE.fullmatch(s):
        return None
    try:
        return datetime.date.fromisoformat(s)
    except ValueError:
        return None


def parse_qty(s):
    if not QTY_RE.fullmatch(s):
        return None
    q = int(s)
    return q if q > 0 else None


def main():
    stock = Stock()
    orders = Orders()
    today = None
    shipped_total = 0
    out = []

    for n, raw in enumerate(sys.stdin, 1):
        line = raw.strip()
        if not line:
            continue
        tok = line.split()
        cmd = tok[0]

        if cmd == 'DAY' and len(tok) == 2:
            d = parse_date(tok[1])
            if d is None:
                out.append('line %d: bad line' % n)
            elif today is not None and d < today:
                out.append('line %d: date goes back' % n)
            else:
                today = d
                for lot in stock.discard_before(today):
                    out.append('discard %s %d %s' % (lot['sku'], lot['qty'],
                                                     lot['expiry'].isoformat()))

        elif cmd == 'RECV' and len(tok) == 4:
            sku = tok[1]
            qty = parse_qty(tok[2])
            expiry = parse_date(tok[3])
            if qty is None or expiry is None:
                out.append('line %d: bad line' % n)
            elif today is None:
                out.append('line %d: no date' % n)
            elif expiry < today:
                out.append('line %d: expired on arrival' % n)
            else:
                stock.add(sku, qty, expiry)
                msgs, got = orders.fill(sku, stock)
                out.extend(msgs)
                shipped_total += got

        elif cmd == 'ORDER' and len(tok) == 4:
            oid, sku = tok[1], tok[2]
            qty = parse_qty(tok[3])
            if qty is None:
                out.append('line %d: bad line' % n)
            elif today is None:
                out.append('line %d: no date' % n)
            elif oid in orders.ids:
                out.append('line %d: duplicate order' % n)
            else:
                orders.ids.add(oid)
                got = stock.take(sku, qty)
                shipped_total += got
                if got == qty:
                    out.append('ORDER %s: shipped %d' % (oid, qty))
                else:
                    out.append('ORDER %s: shipped %d of %d' % (oid, got, qty))
                    orders.add_backorder(oid, sku, qty - got)

        elif cmd == 'STOCK' and len(tok) == 1:
            rows = stock.summary()
            if not rows:
                out.append('stock empty')
            for sku, qty, lots in rows:
                out.append('%s %d in %d lots' % (sku, qty, lots))

        else:
            out.append('line %d: bad line' % n)

    out.append('shipped %d units, backordered %d units' % (shipped_total, orders.waiting()))
    print('\n'.join(out))


if __name__ == '__main__':
    main()
