import sys, re

def fmt(c):
    return "%d.%02d" % (c // 100, c % 100)

def main():
    used = set()
    asks = []  # [price, seq, id, qty]
    bids = []
    seq = 0
    vol = 0
    val = 0
    out = []
    pr = re.compile(r'^(\d+)(?:\.(\d{1,2}))?$')
    for n, line in enumerate(sys.stdin.read().split('\n'), 1):
        p = line.split()
        if not p:
            continue
        cmd = p[0]
        bad = False
        price = None
        qty = None
        if cmd in ('BUY', 'SELL') and len(p) == 4:
            m = pr.match(p[3])
            if not (re.match(r'^\w+$', p[1]) and re.match(r'^\d+$', p[2]) and int(p[2]) >= 1 and m):
                bad = True
            else:
                price = int(m.group(1)) * 100 + int((m.group(2) or '0').ljust(2, '0'))
                if price <= 0:
                    bad = True
                qty = int(p[2])
        elif cmd in ('MBUY', 'MSELL') and len(p) == 3:
            if not (re.match(r'^\w+$', p[1]) and re.match(r'^\d+$', p[2]) and int(p[2]) >= 1):
                bad = True
            else:
                qty = int(p[2])
        elif cmd == 'CANCEL' and len(p) == 2:
            if not re.match(r'^\w+$', p[1]):
                bad = True
        elif cmd == 'BOOK' and len(p) == 1:
            pass
        else:
            bad = True
        if bad:
            out.append("ERR line %d: bad command" % n)
            continue
        if cmd == 'BOOK':
            for name, side in (('asks', asks), ('bids', bids)):
                out.append(name)
                lv = {}
                for o in side:
                    lv[o[0]] = lv.get(o[0], 0) + o[3]
                if not lv:
                    out.append("  -")
                for pc in sorted(lv, reverse=(name == 'bids')):
                    out.append("  %s %d" % (fmt(pc), lv[pc]))
            continue
        if cmd == 'CANCEL':
            oid = p[1]
            found = None
            for side in (asks, bids):
                for o in side:
                    if o[2] == oid:
                        found = (side, o)
            if not found:
                out.append("ERR line %d: unknown ID" % n)
            else:
                found[0].remove(found[1])
                out.append("CANCELLED %s %d" % (oid, found[1][3]))
            continue
        oid = p[1]
        if oid in used:
            out.append("ERR line %d: duplicate ID" % n)
            continue
        used.add(oid)
        seq += 1
        buy = cmd in ('BUY', 'MBUY')
        mkt = cmd in ('MBUY', 'MSELL')
        book = asks if buy else bids
        while qty > 0 and book:
            if buy:
                best = min(book, key=lambda o: (o[0], o[1]))
                if not mkt and best[0] > price:
                    break
            else:
                best = min(book, key=lambda o: (-o[0], o[1]))
                if not mkt and best[0] < price:
                    break
            q = min(qty, best[3])
            qty -= q
            best[3] -= q
            if buy:
                out.append("TRADE %s %s %d %s" % (oid, best[2], q, fmt(best[0])))
            else:
                out.append("TRADE %s %s %d %s" % (best[2], oid, q, fmt(best[0])))
            vol += q
            val += q * best[0]
            if best[3] == 0:
                book.remove(best)
        if qty > 0:
            if mkt:
                out.append("DROPPED %s %d" % (oid, qty))
            else:
                (bids if buy else asks).append([price, seq, oid, qty])
    out.append("volume %d value %s" % (vol, fmt(val)))
    print("\n".join(out))

main()
