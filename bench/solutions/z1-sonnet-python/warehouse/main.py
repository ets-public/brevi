import sys, re, datetime
from stock import Stock
from orders import Orders

def vd(d):
    if not re.match(r'^\d{4}-\d{2}-\d{2}$', d):
        return False
    try:
        datetime.date(int(d[:4]), int(d[5:7]), int(d[8:]))
        return True
    except ValueError:
        return False

def vq(q):
    return re.match(r'^\d+$', q) is not None and int(q) > 0

st = Stock()
od = Orders()
today = None
shipped = 0
out = []
for n, raw in enumerate(sys.stdin.read().split('\n'), 1):
    w = raw.split()
    if not w:
        continue
    c = w[0]
    bad = 'line %d: bad line' % n
    if c == 'DAY':
        if len(w) != 2 or not vd(w[1]):
            out.append(bad)
        elif today is not None and w[1] < today:
            out.append('line %d: date goes back' % n)
        else:
            today = w[1]
            for l in st.expire(today):
                out.append('discard %s %d %s' % (l[0], l[3], l[1]))
    elif c == 'RECV':
        if len(w) != 4 or not vq(w[2]) or not vd(w[3]):
            out.append(bad)
        elif today is None:
            out.append('line %d: no date' % n)
        elif w[3] < today:
            out.append('line %d: expired on arrival' % n)
        else:
            st.add(w[1], int(w[2]), w[3])
            shipped += od.fill(w[1], st, out)
    elif c == 'ORDER':
        if len(w) != 4 or not vq(w[3]):
            out.append(bad)
        elif today is None:
            out.append('line %d: no date' % n)
        elif od.seen(w[1]):
            out.append('line %d: duplicate order' % n)
        else:
            q = int(w[3])
            got = st.take(w[2], q)
            shipped += got
            od.add(w[1], w[2], q, got)
            if got == q:
                out.append('ORDER %s: shipped %d' % (w[1], q))
            else:
                out.append('ORDER %s: shipped %d of %d' % (w[1], got, q))
    elif c == 'STOCK':
        if len(w) != 1:
            out.append(bad)
        else:
            s = st.summary()
            if not s:
                out.append('stock empty')
            for sku, (q, l) in s:
                out.append('%s %d in %d lots' % (sku, q, l))
    else:
        out.append(bad)
out.append('shipped %d units, backordered %d units' % (shipped, od.waiting()))
print('\n'.join(out))
