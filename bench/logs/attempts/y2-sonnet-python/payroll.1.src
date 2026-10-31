import sys, re, datetime
from fractions import Fraction

DEC = r'\d+(?:\.\d{1,2})?'

def fr(s):
    return Fraction(s)

def rhu(x):
    return int((x * 100 + Fraction(1, 2)) // 1)

def money(c):
    return '%d.%02d' % (c // 100, c % 100)

def valid_date(s):
    m = re.fullmatch(r'(\d{4})-(\d\d)-(\d\d)', s)
    if not m:
        return None
    try:
        return datetime.date(int(m.group(1)), int(m.group(2)), int(m.group(3)))
    except ValueError:
        return None

def valid_month(s):
    m = re.fullmatch(r'(\d{4})-(\d\d)', s)
    return bool(m) and 1 <= int(m.group(2)) <= 12

def tax_of(g):
    g = Fraction(g, 100)
    t = Fraction(0)
    if g > 1000:
        t += (min(g, 4000) - 1000) * Fraction(1, 5)
    if g > 4000:
        t += (g - 4000) * Fraction(2, 5)
    return rhu(t)

emps = {}
hours = {}
bonus = {}

def main():
    lines = sys.stdin.read().split('\n')
    for n, raw in enumerate(lines, 1):
        t = raw.split()
        if not t:
            continue
        op = t[0]
        bad = 'line %d: bad line' % n
        if op == 'EMP' and len(t) == 5 and t[3] in ('hourly', 'salary') and re.fullmatch(DEC, t[4]) and fr(t[4]) > 0:
            if t[1] in emps:
                print('line %d: duplicate ID' % n)
                continue
            emps[t[1]] = (t[2], t[3], fr(t[4]))
        elif op == 'HOURS' and len(t) == 4 and valid_date(t[2]) and re.fullmatch(DEC, t[3]) and fr(t[3]) > 0:
            if t[1] not in emps:
                print('line %d: unknown ID' % n)
                continue
            if emps[t[1]][1] != 'hourly':
                print('line %d: not hourly' % n)
                continue
            d = valid_date(t[2])
            key = (t[1], d)
            if hours.get(key, 0) + fr(t[3]) > 24:
                print('line %d: too many hours' % n)
                continue
            hours[key] = hours.get(key, 0) + fr(t[3])
        elif op == 'BONUS' and len(t) == 4 and valid_month(t[2]) and re.fullmatch(DEC, t[3]) and fr(t[3]) > 0:
            if t[1] not in emps:
                print('line %d: unknown ID' % n)
                continue
            bonus[(t[1], t[2])] = bonus.get((t[1], t[2]), 0) + fr(t[3])
        elif op == 'RUN' and len(t) == 2 and valid_month(t[1]):
            run(t[1])
        else:
            print(bad)

def run(month):
    print('payroll ' + month)
    tg = tt = tn = 0
    for eid in sorted(emps):
        name, kind, rate = emps[eid]
        pay = Fraction(0)
        if kind == 'salary':
            pay = rate / 12
        else:
            for (i, d), h in hours.items():
                if i != eid or d.strftime('%Y-%m') != month:
                    continue
                if d.weekday() >= 5:
                    pay += 2 * rate * h
                else:
                    pay += rate * min(h, 8) + Fraction(3, 2) * rate * max(h - 8, 0)
        pay += bonus.get((eid, month), 0)
        g = rhu(pay)
        t = tax_of(g)
        nt = g - t
        tg += g; tt += t; tn += nt
        print('  %s %s gross %s tax %s net %s' % (eid, name, money(g), money(t), money(nt)))
    print('  total gross %s tax %s net %s' % (money(tg), money(tt), money(tn)))

main()
