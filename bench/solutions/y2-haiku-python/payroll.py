import sys
import re
import datetime
from decimal import Decimal, ROUND_HALF_UP

CENT = Decimal('0.01')
ZERO = Decimal(0)
DEC_RE = re.compile(r'[0-9]+(\.[0-9]{1,2})?')
DATE_RE = re.compile(r'[0-9]{4}-[0-9]{2}-[0-9]{2}')
MONTH_RE = re.compile(r'[0-9]{4}-[0-9]{2}')


def pos_dec(s):
    if not DEC_RE.fullmatch(s):
        return None
    v = Decimal(s)
    return v if v > 0 else None


def parse_date(s):
    if not DATE_RE.fullmatch(s):
        return None
    try:
        return datetime.date.fromisoformat(s)
    except ValueError:
        return None


def parse_month(s):
    if not MONTH_RE.fullmatch(s):
        return None
    y, m = int(s[:4]), int(s[5:])
    if 1 <= m <= 12:
        return (y, m)
    return None


def cents(x):
    return x.quantize(CENT, rounding=ROUND_HALF_UP)


def tax_of(g):
    t = ZERO
    if g > 1000:
        t += (min(g, Decimal(4000)) - 1000) * Decimal('0.2')
    if g > 4000:
        t += (g - 4000) * Decimal('0.4')
    return t


emps = {}      # id -> (name, kind, amount)
hours = {}     # id -> {date: Decimal}
bonuses = []   # (id, (y, m), amount)


def run(mo):
    y, m = mo
    out = [f"payroll {y:04d}-{m:02d}"]
    tg = tt = tn = ZERO
    for eid in sorted(emps):
        name, kind, amt = emps[eid]
        if kind == 'salary':
            pay = amt / 12
        else:
            pay = ZERO
            for d, h in hours.get(eid, {}).items():
                if d.year == y and d.month == m:
                    if d.weekday() >= 5:
                        pay += h * 2 * amt
                    else:
                        reg = min(h, Decimal(8))
                        over = h - reg
                        pay += reg * amt + over * amt * Decimal('1.5')
        bon = ZERO
        for bid, bm, b in bonuses:
            if bid == eid and bm == mo:
                bon += b
        gross = cents(pay + bon)
        tax = cents(tax_of(gross))
        net = gross - tax
        out.append(f"  {eid} {name} gross {gross:.2f} tax {tax:.2f} net {net:.2f}")
        tg += gross
        tt += tax
        tn += net
    out.append(f"  total gross {tg:.2f} tax {tt:.2f} net {tn:.2f}")
    print("\n".join(out))


def err(n, msg):
    print(f"line {n}: {msg}")


def main():
    lines = sys.stdin.read().splitlines()
    for n, raw in enumerate(lines, 1):
        tok = raw.split()
        if not tok:
            continue
        cmd = tok[0]
        if cmd == 'EMP' and len(tok) == 5 and tok[3] in ('hourly', 'salary'):
            eid, name, kind, val = tok[1], tok[2], tok[3], tok[4]
            amt = pos_dec(val)
            if amt is None:
                err(n, 'bad line')
            elif eid in emps:
                err(n, 'duplicate ID')
            else:
                emps[eid] = (name, kind, amt)
        elif cmd == 'HOURS' and len(tok) == 4:
            eid, ds, hs = tok[1], tok[2], tok[3]
            d = parse_date(ds)
            h = pos_dec(hs)
            if d is None or h is None:
                err(n, 'bad line')
            elif eid not in emps:
                err(n, 'unknown ID')
            elif emps[eid][1] != 'hourly':
                err(n, 'not hourly')
            else:
                day = hours.setdefault(eid, {})
                total = day.get(d, ZERO) + h
                if total > 24:
                    err(n, 'too many hours')
                else:
                    day[d] = total
        elif cmd == 'BONUS' and len(tok) == 4:
            eid, ms, as_ = tok[1], tok[2], tok[3]
            mo = parse_month(ms)
            amt = pos_dec(as_)
            if mo is None or amt is None:
                err(n, 'bad line')
            elif eid not in emps:
                err(n, 'unknown ID')
            else:
                bonuses.append((eid, mo, amt))
        elif cmd == 'RUN' and len(tok) == 2:
            mo = parse_month(tok[1])
            if mo is None:
                err(n, 'bad line')
            else:
                run(mo)
        else:
            err(n, 'bad line')


if __name__ == "__main__":
    main()
