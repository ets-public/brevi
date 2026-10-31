import sys, re, heapq

PRICE = re.compile(r'^\d+(\.\d{1,2})?$')
QTY = re.compile(r'^\d+$')
WORD = re.compile(r'^\w+$')

def cents(s):
    if '.' in s:
        a, b = s.split('.')
        return int(a) * 100 + int((b + '0')[:2])
    return int(s) * 100

def fmt(c):
    return "%d.%02d" % (c // 100, c % 100)

def main():
    used = set()
    resting = {}  # id -> [qty, side, price, seq]
    buys = []  # heap (-price, seq, id)
    sells = []  # heap (price, seq, id)
    seq = 0
    vol = 0
    val = 0
    out = []

    def match(oid, side, qty, limit):
        nonlocal vol, val
        heap = sells if side == 'B' else buys
        while qty > 0 and heap:
            p, s, rid = heap[0]
            r = resting.get(rid)
            if r is None or r[3] != s:
                heapq.heappop(heap)
                continue
            price = p if side == 'B' else -p
            if limit is not None:
                if side == 'B' and price > limit:
                    break
                if side == 'S' and price < limit:
                    break
            t = min(qty, r[0])
            if side == 'B':
                out.append("TRADE %s %s %d %s" % (oid, rid, t, fmt(price)))
            else:
                out.append("TRADE %s %s %d %s" % (rid, oid, t, fmt(price)))
            vol += t
            val += t * price
            qty -= t
            r[0] -= t
            if r[0] == 0:
                del resting[rid]
                heapq.heappop(heap)
        return qty

    for n, line in enumerate(sys.stdin.read().split('\n'), 1):
        parts = line.split()
        if not parts:
            continue
        c = parts[0]
        ok = False
        if c in ('BUY', 'SELL') and len(parts) == 4:
            ok = bool(WORD.match(parts[1]) and QTY.match(parts[2]) and int(parts[2]) >= 1
                      and PRICE.match(parts[3]) and cents(parts[3]) > 0)
        elif c in ('MBUY', 'MSELL') and len(parts) == 3:
            ok = bool(WORD.match(parts[1]) and QTY.match(parts[2]) and int(parts[2]) >= 1)
        elif c == 'CANCEL' and len(parts) == 2:
            ok = bool(WORD.match(parts[1]))
        elif c == 'BOOK' and len(parts) == 1:
            ok = True
        if not ok:
            out.append("ERR line %d: bad command" % n)
            continue
        if c == 'BOOK':
            for name, side in (('asks', 'S'), ('bids', 'B')):
                out.append(name)
                agg = {}
                for r in resting.values():
                    if r[1] == side:
                        agg[r[2]] = agg.get(r[2], 0) + r[0]
                if not agg:
                    out.append("  -")
                for p in sorted(agg, reverse=(side == 'B')):
                    out.append("  %s %d" % (fmt(p), agg[p]))
            continue
        if c == 'CANCEL':
            r = resting.pop(parts[1], None)
            if r is None:
                out.append("ERR line %d: unknown ID" % n)
            else:
                out.append("CANCELLED %s %d" % (parts[1], r[0]))
            continue
        oid = parts[1]
        if oid in used:
            out.append("ERR line %d: duplicate ID" % n)
            continue
        used.add(oid)
        qty = int(parts[2])
        side = 'B' if c in ('BUY', 'MBUY') else 'S'
        limit = cents(parts[3]) if c in ('BUY', 'SELL') else None
        qty = match(oid, side, qty, limit)
        if qty > 0:
            if limit is None:
                out.append("DROPPED %s %d" % (oid, qty))
            else:
                seq += 1
                resting[oid] = [qty, side, limit, seq]
                if side == 'B':
                    heapq.heappush(buys, (-limit, seq, oid))
                else:
                    heapq.heappush(sells, (limit, seq, oid))
    out.append("volume %d value %s" % (vol, fmt(val)))
    print("\n".join(out))

main()
