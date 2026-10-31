import sys
import re

PRICE_RE = re.compile(r'[0-9]+(\.[0-9]{1,2})?')
QTY_RE = re.compile(r'[0-9]+')


def fmt(c):
    return "%d.%02d" % (c // 100, c % 100)


def parse_price(s):
    if not PRICE_RE.fullmatch(s):
        return None
    if '.' in s:
        a, b = s.split('.')
    else:
        a, b = s, ''
    b = (b + '00')[:2]
    c = int(a) * 100 + int(b)
    return c if c > 0 else None


def parse_qty(s):
    if not QTY_RE.fullmatch(s):
        return None
    q = int(s)
    return q if q >= 1 else None


def match(S, out, side, oid, qty, limit):
    if side == 'BUY':
        book = S['asks']
        cands = [o for o in book if limit is None or o['price'] <= limit]
        cands.sort(key=lambda o: (o['price'], o['seq']))
    else:
        book = S['bids']
        cands = [o for o in book if limit is None or o['price'] >= limit]
        cands.sort(key=lambda o: (-o['price'], o['seq']))
    for o in cands:
        if qty == 0:
            break
        q = min(qty, o['qty'])
        qty -= q
        o['qty'] -= q
        if side == 'BUY':
            out.append("TRADE %s %s %d %s" % (oid, o['id'], q, fmt(o['price'])))
        else:
            out.append("TRADE %s %s %d %s" % (o['id'], oid, q, fmt(o['price'])))
        S['vol'] += q
        S['val'] += q * o['price']
    book[:] = [o for o in book if o['qty'] > 0]
    if qty > 0:
        if limit is None:
            out.append("DROPPED %s %d" % (oid, qty))
        else:
            S['seq'] += 1
            mine = {'id': oid, 'qty': qty, 'price': limit, 'seq': S['seq']}
            (S['bids'] if side == 'BUY' else S['asks']).append(mine)


def levels(orders, reverse):
    agg = {}
    for o in orders:
        agg[o['price']] = agg.get(o['price'], 0) + o['qty']
    return sorted(agg.items(), reverse=reverse)


def main():
    out = []
    S = {'asks': [], 'bids': [], 'seq': 0, 'vol': 0, 'val': 0}
    used = set()
    for n, raw in enumerate(sys.stdin.read().splitlines(), 1):
        line = raw.strip()
        if not line:
            continue
        t = line.split()
        cmd = t[0]
        bad = "ERR line %d: bad command" % n
        if cmd in ('BUY', 'SELL') and len(t) == 4:
            q = parse_qty(t[2])
            p = parse_price(t[3])
            if q is None or p is None:
                out.append(bad)
                continue
            if t[1] in used:
                out.append("ERR line %d: duplicate ID" % n)
                continue
            used.add(t[1])
            match(S, out, 'BUY' if cmd == 'BUY' else 'SELL', t[1], q, p)
        elif cmd in ('MBUY', 'MSELL') and len(t) == 3:
            q = parse_qty(t[2])
            if q is None:
                out.append(bad)
                continue
            if t[1] in used:
                out.append("ERR line %d: duplicate ID" % n)
                continue
            used.add(t[1])
            match(S, out, 'BUY' if cmd == 'MBUY' else 'SELL', t[1], q, None)
        elif cmd == 'CANCEL' and len(t) == 2:
            oid = t[1]
            found = False
            for key in ('bids', 'asks'):
                for o in S[key]:
                    if o['id'] == oid:
                        found = True
                        out.append("CANCELLED %s %d" % (oid, o['qty']))
                        S[key].remove(o)
                        break
                if found:
                    break
            if not found:
                out.append("ERR line %d: unknown ID" % n)
        elif cmd == 'BOOK' and len(t) == 1:
            out.append('asks')
            lv = levels(S['asks'], False)
            if not lv:
                out.append('  -')
            for p, q in lv:
                out.append("  %s %d" % (fmt(p), q))
            out.append('bids')
            lv = levels(S['bids'], True)
            if not lv:
                out.append('  -')
            for p, q in lv:
                out.append("  %s %d" % (fmt(p), q))
        else:
            out.append(bad)
    out.append("volume %d value %s" % (S['vol'], fmt(S['val'])))
    sys.stdout.write("\n".join(out) + "\n")


if __name__ == '__main__':
    main()
