import sys, re, datetime
from collections import deque

NUM = re.compile(r'^\d+(\.\d{1,2})?$')
DATE = re.compile(r'^\d{4}-\d{2}-\d{2}$')

def cents(s):
    if '.' in s:
        a, b = s.split('.')
        return int(a) * 100 + int((b + '0')[:2])
    return int(s) * 100

def fmt(c):
    sign = '-' if c < 0 else ''
    c = abs(c)
    return "%s%d.%02d" % (sign, c // 100, c % 100)

def parse(parts):
    if len(parts) < 4:
        return None
    d, op, sku, q = parts[:4]
    if not DATE.match(d):
        return None
    try:
        datetime.date(int(d[:4]), int(d[5:7]), int(d[8:]))
    except ValueError:
        return None
    if not re.match(r'^\w+$', sku) or not q.isdigit() or not q.isascii() or int(q) < 1:
        return None
    if op in ('RECEIVE', 'SHIP'):
        if len(parts) != 5 or not NUM.match(parts[4]):
            return None
        return d, op, sku, int(q), cents(parts[4])
    if op == 'LOSS':
        if len(parts) != 4:
            return None
        return d, op, sku, int(q), 0
    return None

def main():
    lots = {}
    st = {}
    months = {}
    latest = ''
    out = []
    for n, line in enumerate(sys.stdin.read().split('\n'), 1):
        parts = line.split()
        if not parts:
            continue
        r = parse(parts)
        if r is None:
            out.append("line %d: bad line" % n)
            continue
        d, op, sku, q, p = r
        if d < latest:
            out.append("line %d: date out of order" % n)
            continue
        if op == 'RECEIVE':
            latest = d
            lots.setdefault(sku, deque()).append([q, p])
            st.setdefault(sku, [0, 0, 0])
            continue
        dq = lots.get(sku, deque())
        have = sum(l[0] for l in dq)
        if q > have:
            out.append("line %d: short %s need %d have %d" % (n, sku, q, have))
            continue
        latest = d
        cost = 0
        need = q
        while need:
            l = dq[0]
            t = min(need, l[0])
            cost += t * l[1]
            l[0] -= t
            need -= t
            if l[0] == 0:
                dq.popleft()
        s = st[sku]
        m = months.setdefault(d[:7], [0, 0])
        if op == 'SHIP':
            rev = q * p
            s[0] += cost
            s[1] += rev
            m[0] += rev - cost
        else:
            s[2] += cost
            m[1] += cost
    tp = tl = 0
    for sku in sorted(lots):
        dq = lots[sku]
        oh = sum(l[0] for l in dq)
        val = sum(l[0] * l[1] for l in dq)
        cogs, rev, loss = st[sku]
        out.append("%s on_hand %d value %s cogs %s revenue %s profit %s loss %s" % (
            sku, oh, fmt(val), fmt(cogs), fmt(rev), fmt(rev - cogs), fmt(loss)))
        tp += rev - cogs
        tl += loss
    out.append("months")
    for mo in sorted(months):
        out.append("  %s profit %s loss %s" % (mo, fmt(months[mo][0]), fmt(months[mo][1])))
    out.append("total profit %s loss %s net %s" % (fmt(tp), fmt(tl), fmt(tp - tl)))
    print("\n".join(out))

main()
