import sys, re, datetime
from collections import deque

def fmt(c):
    s = "-" if c < 0 else ""
    c = abs(c)
    return "%s%d.%02d" % (s, c // 100, c % 100)

PR = re.compile(r'^(\d+)(?:\.(\d{1,2}))?$')

def cents(t):
    m = PR.match(t)
    if not m:
        return None
    return int(m.group(1)) * 100 + int((m.group(2) or '0').ljust(2, '0'))

def main():
    lots = {}
    st = {}
    months = {}
    last = None
    out = []
    for n, line in enumerate(sys.stdin.read().split('\n'), 1):
        p = line.split()
        if not p:
            continue
        bad = True
        if len(p) in (4, 5) and re.match(r'^\d{4}-\d\d-\d\d$', p[0]) and re.match(r'^\d+$', p[3 - 0 if False else 3]) if len(p) >= 4 else False:
            pass
        try:
            ok = len(p) >= 4
            if ok:
                date, op, sku, q = p[0], p[1], p[2], p[3]
                datetime.date(int(date[:4]), int(date[5:7]), int(date[8:10]))
                ok = bool(re.match(r'^\d{4}-\d\d-\d\d$', date)) and bool(re.match(r'^\d+$', q)) and int(q) >= 1
                ok = ok and re.match(r'^\S+$', sku) is not None
                if op in ('RECEIVE', 'SHIP'):
                    ok = ok and len(p) == 5 and cents(p[4]) is not None
                elif op == 'LOSS':
                    ok = ok and len(p) == 4
                else:
                    ok = False
        except ValueError:
            ok = False
        if not ok:
            out.append("line %d: bad line" % n)
            continue
        q = int(q)
        if last is not None and date < last:
            out.append("line %d: date out of order" % n)
            continue
        mo = date[:7]
        if op == 'RECEIVE':
            c = cents(p[4])
            lots.setdefault(sku, deque()).append([q, c])
            st.setdefault(sku, [0, 0, 0])
            last = date
            continue
        have = sum(l[0] for l in lots.get(sku, ()))
        if q > have:
            out.append("line %d: short %s need %d have %d" % (n, sku, q, have))
            continue
        last = date
        cost = 0
        need = q
        d = lots[sku]
        while need:
            l = d[0]
            t = min(need, l[0])
            cost += t * l[1]
            l[0] -= t
            need -= t
            if l[0] == 0:
                d.popleft()
        m = months.setdefault(mo, [0, 0])
        s = st[sku]
        if op == 'SHIP':
            rev = q * cents(p[4])
            s[0] += cost
            s[1] += rev
            m[0] += rev - cost
        else:
            s[2] += cost
            m[1] += cost
    tp = tl = 0
    for sku in sorted(st):
        cg, rv, ls = st[sku]
        oh = sum(l[0] for l in lots[sku])
        val = sum(l[0] * l[1] for l in lots[sku])
        out.append("%s on_hand %d value %s cogs %s revenue %s profit %s loss %s" % (sku, oh, fmt(val), fmt(cg), fmt(rv), fmt(rv - cg), fmt(ls)))
        tp += rv - cg
        tl += ls
    out.append("months")
    for mo in sorted(months):
        out.append("  %s profit %s loss %s" % (mo, fmt(months[mo][0]), fmt(months[mo][1])))
    out.append("total profit %s loss %s net %s" % (fmt(tp), fmt(tl), fmt(tp - tl)))
    print("\n".join(out))

main()
