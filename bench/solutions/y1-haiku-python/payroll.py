import datetime
import re
import sys
from decimal import Decimal, ROUND_HALF_UP

CENT = Decimal('0.01')
NUM_RE = re.compile(r'[0-9]+(\.[0-9]{1,2})?')
DATE_RE = re.compile(r'[0-9]{4}-[0-9]{2}-[0-9]{2}')
MONTH_RE = re.compile(r'[0-9]{4}-[0-9]{2}')


def money(x):
    return x.quantize(CENT, rounding=ROUND_HALF_UP)


def fmt(x):
    return '%.2f' % money(x)


def pos_num(s):
    if not NUM_RE.fullmatch(s):
        return None
    v = Decimal(s)
    return v if v > 0 else None


def parse_date(s):
    if not DATE_RE.fullmatch(s):
        return None
    try:
        return datetime.date(int(s[0:4]), int(s[5:7]), int(s[8:10]))
    except ValueError:
        return None


def parse_month(s):
    if not MONTH_RE.fullmatch(s):
        return None
    y, m = int(s[0:4]), int(s[5:7])
    if 1 <= m <= 12:
        return (y, m)
    return None


def day_pay(hours, rate, weekend):
    if weekend:
        return hours * 2 * rate
    regular = min(hours, Decimal(8))
    over = max(hours - 8, Decimal(0))
    return regular * rate + over * Decimal('1.5') * rate


def tax_of(gross):
    t = Decimal(0)
    if gross > 1000:
        t += (min(gross, 4000) - 1000) * Decimal('0.20')
    if gross > 4000:
        t += (gross - 4000) * Decimal('0.40')
    return money(t)


def main():
    data = sys.stdin.read()
    lines = data.split('\n')
    emps = {}
    bonuses = []  # (emp_id, (y, m), amount)
    out = []

    for lineno, raw in enumerate(lines, 1):
        raw = raw.rstrip('\r')
        tok = raw.split()
        if not tok:
            continue

        def bad():
            out.append('line %d: bad line' % lineno)

        cmd = tok[0]
        if cmd == 'EMP' and len(tok) == 5 and tok[3] in ('hourly', 'salary'):
            eid, name = tok[1], tok[2]
            if tok[3] == 'hourly':
                val = pos_num(tok[4])
                if val is None:
                    bad()
                    continue
                kind = 'hourly'
            else:
                val = pos_num(tok[4])
                if val is None:
                    bad()
                    continue
                kind = 'salary'
            if eid in emps:
                out.append('line %d: duplicate ID' % lineno)
                continue
            emps[eid] = {'name': name, 'kind': kind, 'val': val, 'days': {}}
        elif cmd == 'HOURS' and len(tok) == 4:
            eid, ds, hs = tok[1], tok[2], tok[3]
            d = parse_date(ds)
            h = pos_num(hs)
            if d is None or h is None:
                bad()
                continue
            if eid not in emps:
                out.append('line %d: unknown ID' % lineno)
                continue
            e = emps[eid]
            if e['kind'] != 'hourly':
                out.append('line %d: not hourly' % lineno)
                continue
            total = e['days'].get(d, Decimal(0)) + h
            if total > 24:
                out.append('line %d: too many hours' % lineno)
                continue
            e['days'][d] = total
        elif cmd == 'BONUS' and len(tok) == 4:
            eid, ms, as_ = tok[1], tok[2], tok[3]
            ym = parse_month(ms)
            a = pos_num(as_)
            if ym is None or a is None:
                bad()
                continue
            if eid not in emps:
                out.append('line %d: unknown ID' % lineno)
                continue
            bonuses.append((eid, ym, a))
        elif cmd == 'RUN' and len(tok) == 2:
            ym = parse_month(tok[1])
            if ym is None:
                bad()
                continue
            out.append('payroll %s' % tok[1])
            tg = tt = tn = Decimal(0)
            for eid in sorted(emps):
                e = emps[eid]
                pay = Decimal(0)
                if e['kind'] == 'salary':
                    pay = e['val'] / 12
                else:
                    for d, h in e['days'].items():
                        if (d.year, d.month) == ym:
                            pay += day_pay(h, e['val'], d.weekday() >= 5)
                for beid, bym, amt in bonuses:
                    if beid == eid and bym == ym:
                        pay += amt
                gross = money(pay)
                tax = tax_of(gross)
                net = gross - tax
                tg += gross
                tt += tax
                tn += net
                out.append('  %s %s gross %s tax %s net %s' % (
                    eid, e['name'], fmt(gross), fmt(tax), fmt(net)))
            out.append('  total gross %s tax %s net %s' % (fmt(tg), fmt(tt), fmt(tn)))
        else:
            bad()

    print('\n'.join(out))


if __name__ == '__main__':
    main()
