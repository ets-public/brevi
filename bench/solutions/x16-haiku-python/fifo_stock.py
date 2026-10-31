import sys
import re
import datetime
from collections import deque

DATE_RE = re.compile(r'([0-9]{4})-([0-9]{2})-([0-9]{2})\Z')
QTY_RE = re.compile(r'[0-9]+\Z')
PRICE_RE = re.compile(r'([0-9]+)(?:\.([0-9]{1,2}))?\Z')


def parse_date(s):
    m = DATE_RE.match(s)
    if not m:
        return None
    try:
        datetime.date(int(m.group(1)), int(m.group(2)), int(m.group(3)))
    except ValueError:
        return None
    return s


def parse_qty(s):
    if not QTY_RE.match(s):
        return None
    q = int(s)
    return q if q >= 1 else None


def parse_price(s):
    m = PRICE_RE.match(s)
    if not m:
        return None
    return int(m.group(1)) * 100 + int((m.group(2) or '').ljust(2, '0'))


def parse(parts):
    """Return (date, op, sku, qty, price_cents_or_None) or None."""
    if len(parts) < 4:
        return None
    date = parse_date(parts[0])
    if date is None:
        return None
    op, sku = parts[1], parts[2]
    q = parse_qty(parts[3])
    if q is None:
        return None
    if op == 'LOSS':
        if len(parts) != 4:
            return None
        return (date, op, sku, q, None)
    if op in ('RECEIVE', 'SHIP'):
        if len(parts) != 5:
            return None
        p = parse_price(parts[4])
        if p is None:
            return None
        return (date, op, sku, q, p)
    return None


def money(c):
    sign = '-' if c < 0 else ''
    c = abs(c)
    return '%s%d.%02d' % (sign, c // 100, c % 100)


def take(dq, q):
    """Remove q units from the oldest lots; return their total cost in cents."""
    cost = 0
    need = q
    while need > 0:
        lot = dq[0]
        t = min(need, lot[0])
        cost += t * lot[1]
        lot[0] -= t
        need -= t
        if lot[0] == 0:
            dq.popleft()
    return cost


def main():
    lots = {}     # sku -> deque of [qty, unit_cost_cents]
    stats = {}    # sku -> dict of totals
    months = {}   # 'YYYY-MM' -> [profit, loss]
    last_date = None
    out = []

    for n, raw in enumerate(sys.stdin.read().split('\n'), 1):
        line = raw.strip()
        if not line:
            continue
        rec = parse(line.split())
        if rec is None:
            out.append('line %d: bad line' % n)
            continue
        date, op, sku, q, price = rec
        if last_date is not None and date < last_date:
            out.append('line %d: date out of order' % n)
            continue

        if op == 'RECEIVE':
            lots.setdefault(sku, deque()).append([q, price])
            if sku not in stats:
                stats[sku] = {'cogs': 0, 'rev': 0, 'profit': 0, 'loss': 0}
            last_date = date
            continue

        dq = lots.get(sku, deque())
        have = sum(lot[0] for lot in dq)
        if have < q:
            out.append('line %d: short %s need %d have %d' % (n, sku, q, have))
            continue

        mon = date[:7]
        m = months.setdefault(mon, [0, 0])
        st = stats[sku]
        if op == 'SHIP':
            cogs = take(dq, q)
            rev = q * price
            prof = rev - cogs
            st['cogs'] += cogs
            st['rev'] += rev
            st['profit'] += prof
            m[0] += prof
        else:  # LOSS
            cost = take(dq, q)
            st['loss'] += cost
            m[1] += cost
        last_date = date

    for sku in sorted(stats):
        st = stats[sku]
        dq = lots[sku]
        onh = sum(lot[0] for lot in dq)
        val = sum(lot[0] * lot[1] for lot in dq)
        out.append('%s on_hand %d value %s cogs %s revenue %s profit %s loss %s' % (
            sku, onh, money(val), money(st['cogs']), money(st['rev']),
            money(st['profit']), money(st['loss'])))

    out.append('months')
    for mon in sorted(months):
        p, l = months[mon]
        out.append('  %s profit %s loss %s' % (mon, money(p), money(l)))

    tp = sum(st['profit'] for st in stats.values())
    tl = sum(st['loss'] for st in stats.values())
    out.append('total profit %s loss %s net %s' % (money(tp), money(tl), money(tp - tl)))

    sys.stdout.write('\n'.join(out) + '\n')


if __name__ == '__main__':
    main()
