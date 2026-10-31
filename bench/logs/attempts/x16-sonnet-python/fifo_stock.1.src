import sys, re, datetime
from collections import deque

DATE = re.compile(r'^[0-9]{4}-[0-9]{2}-[0-9]{2}$')
QTY = re.compile(r'^[0-9]+$')
PRC = re.compile(r'^[0-9]+(\.[0-9]{1,2})?$')

def cents(s):
    if '.' in s:
        a, b = s.split('.')
        b = (b + '00')[:2]
    else:
        a, b = s, '00'
    return int(a) * 100 + int(b)

def fmt(c):
    sign = '-' if c < 0 else ''
    c = abs(c)
    return "%s%d.%02d" % (sign, c // 100, c % 100)

def valid_date(d):
    if not DATE.match(d):
        return False
    try:
        datetime.date(int(d[:4]), int(d[5:7]), int(d[8:]))
        return True
    except ValueError:
        return False

def main():
    lots = {}
    stat = {}
    months = {}
    latest = None
    out = []
    for n, line in enumerate(sys.stdin.read().split('\n'), 1):
        if not line.strip():
            continue
        t = line.split()
        ok = False
        if len(t) >= 3 and valid_date(t[0]):
            op = t[1]
            if op in ('RECEIVE', 'SHIP') and len(t) == 5 and QTY.match(t[3]) and int(t[3]) >= 1 and PRC.match(t[4]):
                ok = True
            elif op == 'LOSS' and len(t) == 4 and QTY.match(t[3]) and int(t[3]) >= 1:
                ok = True
        if not ok:
            out.append("line %d: bad line" % n)
            continue
        date, op, sku = t[0], t[1], t[2]
        qty = int(t[3])
        if latest is not None and date < latest:
            out.append("line %d: date out of order" % n)
            continue
        q = lots.setdefault(sku, deque())
        st = stat.setdefault(sku, {'cogs': 0, 'rev': 0, 'loss': 0, 'seen': False})
        if op == 'RECEIVE':
            q.append([qty, cents(t[4])])
            st['seen'] = True
            latest = date
            continue
        have = sum(l[0] for l in q)
        if qty > have:
            out.append("line %d: short %s need %d have %d" % (n, sku, qty, have))
            continue
        latest = date
        cost = 0
        need = qty
        while need > 0:
            l = q[0]
            take = min(need, l[0])
            cost += take * l[1]
            l[0] -= take
            need -= take
            if l[0] == 0:
                q.popleft()
        m = months.setdefault(date[:7], [0, 0])
        if op == 'SHIP':
            rev = qty * cents(t[4])
            st['cogs'] += cost
            st['rev'] += rev
            m[0] += rev - cost
        else:
            st['loss'] += cost
            m[1] += cost
    tp = tl = 0
    for sku in sorted(stat):
        st = stat[sku]
        if not st['seen']:
            continue
        q = lots[sku]
        onh = sum(l[0] for l in q)
        val = sum(l[0] * l[1] for l in q)
        profit = st['rev'] - st['cogs']
        tp += profit
        tl += st['loss']
        out.append("%s on_hand %d value %s cogs %s revenue %s profit %s loss %s" % (
            sku, onh, fmt(val), fmt(st['cogs']), fmt(st['rev']), fmt(profit), fmt(st['loss'])))
    out.append("months")
    for mo in sorted(months):
        out.append("  %s profit %s loss %s" % (mo, fmt(months[mo][0]), fmt(months[mo][1])))
    out.append("total profit %s loss %s net %s" % (fmt(tp), fmt(tl), fmt(tp - tl)))
    print('\n'.join(out))

main()
