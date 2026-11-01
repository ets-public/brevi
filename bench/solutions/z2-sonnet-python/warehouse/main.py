import sys, re, datetime
from stock import Stock
from orders import Orders


def vdate(d):
    if not re.match(r'^\d{4}-\d{2}-\d{2}$', d):
        return False
    try:
        datetime.date(int(d[:4]), int(d[5:7]), int(d[8:]))
        return True
    except ValueError:
        return False


def vqty(q):
    return re.match(r'^\d+$', q) is not None and int(q) > 0


stock = Stock()
orders = Orders()
today = None
shipped = 0
n = 0
for raw in sys.stdin.read().split('\n'):
    n += 1
    w = raw.split()
    if not w:
        continue
    c = w[0]
    if c == 'DAY' and len(w) == 2 and vdate(w[1]):
        if today is not None and w[1] < today:
            print('line %d: date goes back' % n)
            continue
        today = w[1]
        for sku, q, e in stock.expire(today):
            print('discard %s %d %s' % (sku, q, e))
    elif c == 'RECV' and len(w) == 4 and vqty(w[2]) and vdate(w[3]):
        if today is None:
            print('line %d: no date' % n)
        elif w[3] < today:
            print('line %d: expired on arrival' % n)
        else:
            stock.add(w[1], int(w[2]), w[3])
            for msg, k in orders.fill(w[1], stock):
                print(msg)
                shipped += k
    elif c == 'ORDER' and len(w) == 4 and vqty(w[3]):
        if today is None:
            print('line %d: no date' % n)
        elif orders.known(w[1]):
            print('line %d: duplicate order' % n)
        else:
            orders.register(w[1])
            q = int(w[3])
            got = stock.take(w[2], q)
            shipped += got
            if got == q:
                print('ORDER %s: shipped %d' % (w[1], q))
            else:
                print('ORDER %s: shipped %d of %d' % (w[1], got, q))
                orders.add_back(w[1], w[2], q - got)
    elif c == 'STOCK' and len(w) == 1:
        s = stock.summary()
        if not s:
            print('stock empty')
        for sku, q, l in s:
            print('%s %d in %d lots' % (sku, q, l))
    else:
        print('line %d: bad line' % n)
print('shipped %d units, backordered %d units' % (shipped, orders.waiting()))
