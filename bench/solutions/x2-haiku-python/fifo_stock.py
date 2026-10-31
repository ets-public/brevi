import sys
import re
import datetime
from collections import deque

DATE_RE = re.compile(r'[0-9]{4}-[0-9]{2}-[0-9]{2}')
QTY_RE = re.compile(r'[0-9]+')
PRICE_RE = re.compile(r'[0-9]+(\.[0-9]{1,2})?')


def cents(s):
    if '.' in s:
        a, b = s.split('.')
    else:
        a, b = s, ''
    return int(a) * 100 + int((b + '00')[:2])


def money(c):
    sign = '-' if c < 0 else ''
    c = abs(c)
    return '%s%d.%02d' % (sign, c // 100, c % 100)


def take(dq, qty):
    cost = 0
    while qty > 0:
        lot = dq[0]
        k = min(qty, lot[0])
        cost += k * lot[1]
        lot[0] -= k
        qty -= k
        if lot[0] == 0:
            dq.popleft()
    return cost


def main():
    lots = {}
    stats = {}
    months = {}
    latest = None
    out = []
    for n, raw in enumerate(sys.stdin.read().splitlines(), 1):
        t = raw.split()
        if not t:
            continue
        bad = "line %d: bad line" % n
        if len(t) < 4:
            out.append(bad)
            continue
        date, op, sku = t[0], t[1], t[2]
        if not DATE_RE.fullmatch(date):
            out.append(bad)
            continue
        try:
            d = datetime.date(int(date[:4]), int(date[5:7]), int(date[8:]))
        except ValueError:
            out.append(bad)
            continue
        ok = False
        if not QTY_RE.fullmatch(t[3]) or int(t[3]) < 1:
            ok = False
        else:
            qty = int(t[3])
            if op == 'RECEIVE' and len(t) == 5 and PRICE_RE.fullmatch(t[4]):
                ok = True
                c = cents(t[4])
            elif op == 'SHIP' and len(t) == 5 and PRICE_RE.fullmatch(t[4]):
                ok = True
                price = cents(t[4])
            elif op == 'LOSS' and len(t) == 4:
                ok = True
        if not ok:
            out.append(bad)
            continue
        if latest is not None and d < latest:
            out.append("line %d: date out of order" % n)
            continue
        ym = "%04d-%02d" % (d.year, d.month)
        if op == 'RECEIVE':
            lots.setdefault(sku, deque()).append([qty, c])
            stats.setdefault(sku, {'cogs': 0, 'rev': 0, 'profit': 0, 'loss': 0})
            latest = d
        else:
            dq = lots.get(sku, deque())
            have = sum(lot[0] for lot in dq)
            if qty > have:
                out.append("line %d: short %s need %d have %d" % (n, sku, qty, have))
                continue
            st = stats.setdefault(sku, {'cogs': 0, 'rev': 0, 'profit': 0, 'loss': 0})
            m = months.setdefault(ym, [0, 0])
            latest = d
            if op == 'SHIP':
                cogs = take(dq, qty)
                rev = qty * price
                profit = rev - cogs
                st['cogs'] += cogs
                st['rev'] += rev
                st['profit'] += profit
                m[0] += profit
            else:
                cost = take(dq, qty)
                st['loss'] += cost
                m[1] += cost

    for sku in sorted(stats):
        dq = lots.get(sku, deque())
        on_hand = sum(lot[0] for lot in dq)
        value = sum(lot[0] * lot[1] for lot in dq)
        st = stats[sku]
        out.append("%s on_hand %d value %s cogs %s revenue %s profit %s loss %s" % (
            sku, on_hand, money(value), money(st['cogs']), money(st['rev']),
            money(st['profit']), money(st['loss'])))
    out.append("months")
    for ym in sorted(months):
        out.append("  %s profit %s loss %s" % (ym, money(months[ym][0]), money(months[ym][1])))
    tp = sum(s['profit'] for s in stats.values())
    tl = sum(s['loss'] for s in stats.values())
    out.append("total profit %s loss %s net %s" % (money(tp), money(tl), money(tp - tl)))
    sys.stdout.write("\n".join(out) + "\n")


if __name__ == '__main__':
    main()
