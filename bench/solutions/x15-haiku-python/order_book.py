import re
import sys


def fmt(cents):
    return "%d.%02d" % (cents // 100, cents % 100)


def parse_price(s):
    m = re.fullmatch(r"(\d+)(?:\.(\d{1,2}))?", s)
    if not m:
        return None
    cents = int(m.group(1)) * 100 + int((m.group(2) or "0").ljust(2, "0"))
    return cents if cents > 0 else None


def parse_qty(s):
    if not re.fullmatch(r"\d+", s):
        return None
    q = int(s)
    return q if q >= 1 else None


ID_RE = re.compile(r"\w+")


class Order:
    __slots__ = ("oid", "qty", "price", "seq", "side")

    def __init__(self, oid, qty, price, seq, side):
        self.oid = oid
        self.qty = qty
        self.price = price
        self.seq = seq
        self.side = side


def main():
    bids = []
    asks = []
    resting = {}
    used = set()
    seq = 0
    volume = 0
    value = 0
    out = []

    def match(oid, qty, limit, side):
        nonlocal volume, value
        book = asks if side == "BUY" else bids
        while qty > 0:
            cands = []
            for o in book:
                if limit is None:
                    cands.append(o)
                elif side == "BUY" and o.price <= limit:
                    cands.append(o)
                elif side == "SELL" and o.price >= limit:
                    cands.append(o)
            if not cands:
                break
            if side == "BUY":
                best = min(cands, key=lambda o: (o.price, o.seq))
                buy_id, sell_id = oid, best.oid
            else:
                best = min(cands, key=lambda o: (-o.price, o.seq))
                buy_id, sell_id = best.oid, oid
            q = min(qty, best.qty)
            out.append("TRADE %s %s %d %s" % (buy_id, sell_id, q, fmt(best.price)))
            volume += q
            value += q * best.price
            qty -= q
            best.qty -= q
            if best.qty == 0:
                book.remove(best)
                resting.pop(best.oid, None)
        return qty

    for n, raw in enumerate(sys.stdin, 1):
        line = raw.strip()
        if not line:
            continue
        parts = line.split()
        cmd = parts[0]

        if cmd in ("BUY", "SELL") and len(parts) == 4:
            oid, qs, ps = parts[1], parts[2], parts[3]
            qty = parse_qty(qs)
            price = parse_price(ps)
            if not ID_RE.fullmatch(oid) or qty is None or price is None:
                out.append("ERR line %d: bad command" % n)
                continue
            if oid in used:
                out.append("ERR line %d: duplicate ID" % n)
                continue
            used.add(oid)
            rem = match(oid, qty, price, cmd)
            if rem > 0:
                seq += 1
                o = Order(oid, rem, price, seq, cmd)
                (bids if cmd == "BUY" else asks).append(o)
                resting[oid] = o

        elif cmd in ("MBUY", "MSELL") and len(parts) == 3:
            oid, qs = parts[1], parts[2]
            qty = parse_qty(qs)
            if not ID_RE.fullmatch(oid) or qty is None:
                out.append("ERR line %d: bad command" % n)
                continue
            if oid in used:
                out.append("ERR line %d: duplicate ID" % n)
                continue
            used.add(oid)
            side = "BUY" if cmd == "MBUY" else "SELL"
            rem = match(oid, qty, None, side)
            if rem > 0:
                out.append("DROPPED %s %d" % (oid, rem))

        elif cmd == "CANCEL" and len(parts) == 2:
            oid = parts[1]
            if not ID_RE.fullmatch(oid):
                out.append("ERR line %d: bad command" % n)
                continue
            o = resting.get(oid)
            if o is None:
                out.append("ERR line %d: unknown ID" % n)
                continue
            (bids if o.side == "BUY" else asks).remove(o)
            del resting[oid]
            out.append("CANCELLED %s %d" % (oid, o.qty))

        elif cmd == "BOOK" and len(parts) == 1:
            out.append("asks")
            levels = {}
            for o in asks:
                levels[o.price] = levels.get(o.price, 0) + o.qty
            if not levels:
                out.append("  -")
            for p in sorted(levels):
                out.append("  %s %d" % (fmt(p), levels[p]))
            out.append("bids")
            levels = {}
            for o in bids:
                levels[o.price] = levels.get(o.price, 0) + o.qty
            if not levels:
                out.append("  -")
            for p in sorted(levels, reverse=True):
                out.append("  %s %d" % (fmt(p), levels[p]))

        else:
            out.append("ERR line %d: bad command" % n)

    out.append("volume %d value %s" % (volume, fmt(value)))
    sys.stdout.write("\n".join(out) + "\n")


if __name__ == "__main__":
    main()
