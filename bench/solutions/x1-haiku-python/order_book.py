import re
import sys

INT_RE = re.compile(r'[0-9]+')
PRICE_RE = re.compile(r'[0-9]+(\.[0-9]{1,2})?')


def to_cents(s):
    if '.' in s:
        a, b = s.split('.')
        b = (b + '00')[:2]
    else:
        a, b = s, '00'
    return int(a) * 100 + int(b)


def fmt(c):
    return "%d.%02d" % (c // 100, c % 100)


def main():
    lines = sys.stdin.read().split('\n')
    out = []
    used = set()
    resting = {}  # id -> order dict, only for orders in the book
    book = {'BUY': [], 'SELL': []}
    seq = 0
    vol_q = 0
    vol_v = 0

    for lineno, raw in enumerate(lines, 1):
        line = raw.strip()
        if not line:
            continue
        tok = line.split()
        cmd = tok[0]

        if cmd in ('BUY', 'SELL') and len(tok) == 4:
            oid, qty, price = tok[1], tok[2], tok[3]
            if not (INT_RE.fullmatch(qty) and int(qty) >= 1 and PRICE_RE.fullmatch(price) and to_cents(price) > 0):
                out.append("ERR line %d: bad command" % lineno)
                continue
            if oid in used:
                out.append("ERR line %d: duplicate ID" % lineno)
                continue
            used.add(oid)
            limit = to_cents(price)
            kind = 'limit'
        elif cmd in ('MBUY', 'MSELL') and len(tok) == 3:
            oid, qty = tok[1], tok[2]
            if not (INT_RE.fullmatch(qty) and int(qty) >= 1):
                out.append("ERR line %d: bad command" % lineno)
                continue
            if oid in used:
                out.append("ERR line %d: duplicate ID" % lineno)
                continue
            used.add(oid)
            limit = None
            kind = 'market'
        elif cmd == 'CANCEL' and len(tok) == 2:
            oid = tok[1]
            o = resting.get(oid)
            if o is None:
                out.append("ERR line %d: unknown ID" % lineno)
                continue
            book[o['side']].remove(o)
            del resting[oid]
            out.append("CANCELLED %s %d" % (oid, o['qty']))
            continue
        elif cmd == 'BOOK' and len(tok) == 1:
            out.append('asks')
            asks = {}
            for o in book['SELL']:
                asks[o['price']] = asks.get(o['price'], 0) + o['qty']
            if not asks:
                out.append('  -')
            for p in sorted(asks):
                out.append("  %s %d" % (fmt(p), asks[p]))
            out.append('bids')
            bids = {}
            for o in book['BUY']:
                bids[o['price']] = bids.get(o['price'], 0) + o['qty']
            if not bids:
                out.append('  -')
            for p in sorted(bids, reverse=True):
                out.append("  %s %d" % (fmt(p), bids[p]))
            continue
        else:
            out.append("ERR line %d: bad command" % lineno)
            continue

        # An accepted order: match it against the opposite side.
        side = 'BUY' if cmd in ('BUY', 'MBUY') else 'SELL'
        opp = 'SELL' if side == 'BUY' else 'BUY'
        qty = int(qty)
        rem = qty
        if side == 'BUY':
            cands = [o for o in book[opp] if limit is None or o['price'] <= limit]
            cands.sort(key=lambda o: (o['price'], o['seq']))
        else:
            cands = [o for o in book[opp] if limit is None or o['price'] >= limit]
            cands.sort(key=lambda o: (-o['price'], o['seq']))

        for o in cands:
            if rem == 0:
                break
            f = min(rem, o['qty'])
            if side == 'BUY':
                out.append("TRADE %s %s %d %s" % (oid, o['id'], f, fmt(o['price'])))
            else:
                out.append("TRADE %s %s %d %s" % (o['id'], oid, f, fmt(o['price'])))
            o['qty'] -= f
            rem -= f
            vol_q += f
            vol_v += f * o['price']
            if o['qty'] == 0:
                book[opp].remove(o)
                del resting[o['id']]

        if rem > 0:
            if kind == 'limit':
                seq += 1
                o = {'id': oid, 'side': side, 'price': limit, 'qty': rem, 'seq': seq}
                book[side].append(o)
                resting[oid] = o
            else:
                out.append("DROPPED %s %d" % (oid, rem))

    out.append("volume %d value %s" % (vol_q, fmt(vol_v)))
    sys.stdout.write('\n'.join(out) + '\n')


main()
