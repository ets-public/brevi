import sys, re

def cents(s):
    if '.' in s:
        a, b = s.split('.')
        b = (b + '00')[:2]
    else:
        a, b = s, '00'
    return int(a) * 100 + int(b)

def fmt(c):
    return "%d.%02d" % (c // 100, c % 100)

ID = re.compile(r'^\w+$')
QTY = re.compile(r'^[0-9]+$')
PRC = re.compile(r'^[0-9]+(\.[0-9]{1,2})?$')

def main():
    lines = sys.stdin.read().split('\n')
    seen = set()
    asks = []  # orders: [seq, id, qty, price]
    bids = []
    seq = 0
    vol = 0
    val = 0
    out = []

    def match(side, oid, qty, limit):
        nonlocal vol, val
        book = asks if side == 'B' else bids
        while qty > 0 and book:
            if side == 'B':
                best = min(book, key=lambda o: (o[3], o[0]))
                if limit is not None and best[3] > limit:
                    break
            else:
                best = min(book, key=lambda o: (-o[3], o[0]))
                if limit is not None and best[3] < limit:
                    break
            q = min(qty, best[2])
            qty -= q
            best[2] -= q
            if best[2] == 0:
                book.remove(best)
            if side == 'B':
                out.append("TRADE %s %s %d %s" % (oid, best[1], q, fmt(best[3])))
            else:
                out.append("TRADE %s %s %d %s" % (best[1], oid, q, fmt(best[3])))
            vol += q
            val += q * best[3]
        return qty

    for n, line in enumerate(lines, 1):
        if not line.strip():
            continue
        t = line.split()
        cmd = t[0]
        ok = False
        if cmd in ('BUY', 'SELL') and len(t) == 4 and ID.match(t[1]) and QTY.match(t[2]) and int(t[2]) >= 1 and PRC.match(t[3]) and cents(t[3]) > 0:
            ok = True
        elif cmd in ('MBUY', 'MSELL') and len(t) == 3 and ID.match(t[1]) and QTY.match(t[2]) and int(t[2]) >= 1:
            ok = True
        elif cmd == 'CANCEL' and len(t) == 2 and ID.match(t[1]):
            ok = True
        elif cmd == 'BOOK' and len(t) == 1:
            ok = True
        if not ok:
            out.append("ERR line %d: bad command" % n)
            continue
        if cmd == 'BOOK':
            for name, book, rev in (('asks', asks, False), ('bids', bids, True)):
                out.append(name)
                tot = {}
                for o in book:
                    tot[o[3]] = tot.get(o[3], 0) + o[2]
                if not tot:
                    out.append("  -")
                for p in sorted(tot, reverse=rev):
                    out.append("  %s %d" % (fmt(p), tot[p]))
            continue
        if cmd == 'CANCEL':
            found = None
            for book in (asks, bids):
                for o in book:
                    if o[1] == t[1]:
                        found = (book, o)
            if not found:
                out.append("ERR line %d: unknown ID" % n)
            else:
                found[0].remove(found[1])
                out.append("CANCELLED %s %d" % (t[1], found[1][2]))
            continue
        oid = t[1]
        if oid in seen:
            out.append("ERR line %d: duplicate ID" % n)
            continue
        seen.add(oid)
        qty = int(t[2])
        side = 'B' if cmd in ('BUY', 'MBUY') else 'S'
        market = cmd.startswith('M')
        limit = None if market else cents(t[3])
        seq += 1
        rest = match(side, oid, qty, limit)
        if rest > 0:
            if market:
                out.append("DROPPED %s %d" % (oid, rest))
            else:
                (bids if side == 'B' else asks).append([seq, oid, rest, limit])
    out.append("volume %d value %s" % (vol, fmt(val)))
    print('\n'.join(out))

main()
