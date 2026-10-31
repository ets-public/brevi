import sys
import re
from collections import deque

ID_RE = re.compile(r'[A-Za-z0-9_]+\Z')
QTY_RE = re.compile(r'[0-9]+\Z')
PRICE_RE = re.compile(r'([0-9]+)(?:\.([0-9]{1,2}))?\Z')


def fmt(c):
    return "%d.%02d" % (c // 100, c % 100)


def parse_qty(s):
    if not QTY_RE.match(s):
        return None
    q = int(s)
    return q if q >= 1 else None


def parse_price(s):
    m = PRICE_RE.match(s)
    if not m:
        return None
    c = int(m.group(1)) * 100 + int((m.group(2) or '').ljust(2, '0'))
    return c if c > 0 else None


def parse(parts):
    """Return (kind, id, qty, price_cents) or None if the line is invalid."""
    cmd = parts[0]
    if cmd in ('BUY', 'SELL') and len(parts) == 4:
        if not ID_RE.match(parts[1]):
            return None
        q = parse_qty(parts[2])
        p = parse_price(parts[3])
        if q is None or p is None:
            return None
        return ('limit', parts[1], q, p)
    if cmd in ('MBUY', 'MSELL') and len(parts) == 3:
        if not ID_RE.match(parts[1]):
            return None
        q = parse_qty(parts[2])
        if q is None:
            return None
        return ('market', parts[1], q, None)
    if cmd == 'CANCEL' and len(parts) == 2:
        if not ID_RE.match(parts[1]):
            return None
        return ('cancel', parts[1], None, None)
    if cmd == 'BOOK' and len(parts) == 1:
        return ('book', None, None, None)
    return None


def main():
    bids = {}    # price_cents -> deque of [id, qty] in arrival order
    asks = {}
    where = {}   # resting id -> (is_buy, price_cents)
    seen = set()
    out = []
    vol = 0
    val = 0

    for n, raw in enumerate(sys.stdin.read().split('\n'), 1):
        line = raw.strip()
        if not line:
            continue
        parsed = parse(line.split())
        if parsed is None:
            out.append("ERR line %d: bad command" % n)
            continue
        kind, oid, qty, price = parsed

        if kind == 'book':
            out.append('asks')
            if not asks:
                out.append('  -')
            for px in sorted(asks):
                out.append("  %s %d" % (fmt(px), sum(r[1] for r in asks[px])))
            out.append('bids')
            if not bids:
                out.append('  -')
            for px in sorted(bids, reverse=True):
                out.append("  %s %d" % (fmt(px), sum(r[1] for r in bids[px])))
            continue

        if kind == 'cancel':
            if oid not in where:
                out.append("ERR line %d: unknown ID" % n)
                continue
            is_buy, px = where.pop(oid)
            levels = bids if is_buy else asks
            dq = levels[px]
            left = 0
            for i, r in enumerate(dq):
                if r[0] == oid:
                    left = r[1]
                    del dq[i]
                    break
            if not dq:
                del levels[px]
            out.append("CANCELLED %s %d" % (oid, left))
            continue

        # BUY / SELL / MBUY / MSELL
        if oid in seen:
            out.append("ERR line %d: duplicate ID" % n)
            continue
        seen.add(oid)
        cmd = line.split()[0]
        is_buy = cmd in ('BUY', 'MBUY')
        opp = asks if is_buy else bids
        prices = sorted(opp) if is_buy else sorted(opp, reverse=True)
        rem = qty

        for px in prices:
            if rem == 0:
                break
            if price is not None:
                if is_buy and px > price:
                    break
                if not is_buy and px < price:
                    break
            dq = opp[px]
            while dq and rem > 0:
                r = dq[0]
                f = min(rem, r[1])
                if is_buy:
                    out.append("TRADE %s %s %d %s" % (oid, r[0], f, fmt(px)))
                else:
                    out.append("TRADE %s %s %d %s" % (r[0], oid, f, fmt(px)))
                vol += f
                val += f * px
                r[1] -= f
                rem -= f
                if r[1] == 0:
                    dq.popleft()
                    del where[r[0]]
            if not dq:
                del opp[px]

        if rem > 0:
            if price is None:
                out.append("DROPPED %s %d" % (oid, rem))
            else:
                mine = bids if is_buy else asks
                mine.setdefault(price, deque()).append([oid, rem])
                where[oid] = (is_buy, price)

    out.append("volume %d value %s" % (vol, fmt(val)))
    sys.stdout.write('\n'.join(out) + '\n')


if __name__ == '__main__':
    main()
