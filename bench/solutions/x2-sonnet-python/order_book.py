import sys, re

def money(c):
    return "%d.%02d" % (c // 100, c % 100)

def parse_price(s):
    if not re.fullmatch(r"\d+(\.\d{1,2})?", s):
        return None
    if '.' in s:
        a, b = s.split('.')
        c = int(a) * 100 + int(b.ljust(2, '0'))
    else:
        c = int(s) * 100
    return c if c > 0 else None

def main():
    asks = []  # [price, seq, id, qty]
    bids = []
    used = set()
    seq = 0
    vol = 0
    val = 0
    out = []
    lines = sys.stdin.read().split('\n')
    if lines and lines[-1] == '':
        lines.pop()
    for n, line in enumerate(lines, 1):
        t = line.split()
        if not t:
            continue
        cmd = t[0]
        order = None
        if cmd in ("BUY", "SELL") and len(t) == 4:
            p = parse_price(t[3])
            if re.fullmatch(r"\w+", t[1]) and re.fullmatch(r"\d+", t[2]) and int(t[2]) >= 1 and p is not None:
                order = (cmd == "BUY", t[1], int(t[2]), p)
        elif cmd in ("MBUY", "MSELL") and len(t) == 3:
            if re.fullmatch(r"\w+", t[1]) and re.fullmatch(r"\d+", t[2]) and int(t[2]) >= 1:
                order = (cmd == "MBUY", t[1], int(t[2]), None)
        elif cmd == "CANCEL" and len(t) == 2 and re.fullmatch(r"\w+", t[1]):
            found = None
            for side in (asks, bids):
                for o in side:
                    if o[2] == t[1]:
                        found = (side, o)
            if not found:
                out.append("ERR line %d: unknown ID" % n)
            else:
                found[0].remove(found[1])
                out.append("CANCELLED %s %d" % (t[1], found[1][3]))
            continue
        elif cmd == "BOOK" and len(t) == 1:
            for name, side, rev in (("asks", asks, False), ("bids", bids, True)):
                out.append(name)
                agg = {}
                for o in side:
                    agg[o[0]] = agg.get(o[0], 0) + o[3]
                if not agg:
                    out.append("  -")
                for p in sorted(agg, reverse=rev):
                    out.append("  %s %d" % (money(p), agg[p]))
            continue
        if order is None:
            out.append("ERR line %d: bad command" % n)
            continue
        isbuy, oid, qty, price = order
        if oid in used:
            out.append("ERR line %d: duplicate ID" % n)
            continue
        used.add(oid)
        seq += 1
        opp = asks if isbuy else bids
        while qty > 0 and opp:
            if isbuy:
                best = min(opp, key=lambda o: (o[0], o[1]))
                if price is not None and best[0] > price:
                    break
            else:
                best = min(opp, key=lambda o: (-o[0], o[1]))
                if price is not None and best[0] < price:
                    break
            q = min(qty, best[3])
            if isbuy:
                out.append("TRADE %s %s %d %s" % (oid, best[2], q, money(best[0])))
            else:
                out.append("TRADE %s %s %d %s" % (best[2], oid, q, money(best[0])))
            vol += q
            val += q * best[0]
            qty -= q
            best[3] -= q
            if best[3] == 0:
                opp.remove(best)
        if qty > 0:
            if price is None:
                out.append("DROPPED %s %d" % (oid, qty))
            else:
                (bids if isbuy else asks).append([price, seq, oid, qty])
    out.append("volume %d value %s" % (vol, money(val)))
    print("\n".join(out))

main()
