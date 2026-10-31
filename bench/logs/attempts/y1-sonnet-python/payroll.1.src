import sys, re, datetime
from fractions import Fraction
from math import floor

DEC = re.compile(r'^\d+(\.\d{1,2})?$')
MON = re.compile(r'^\d{4}-(0[1-9]|1[0-2])$')

def dec(s):
    if not DEC.match(s): return None
    v = Fraction(s)
    return v if v > 0 else None

def date(s):
    if not re.match(r'^\d{4}-\d{2}-\d{2}$', s): return None
    try:
        return datetime.date(int(s[:4]), int(s[5:7]), int(s[8:]))
    except ValueError:
        return None

def rnd(x):
    return floor(x * 100 + Fraction(1, 2))

def fmt(c):
    return "%d.%02d" % (c // 100, c % 100)

def tax(g):
    g = Fraction(g, 100)
    t = Fraction(0)
    if g > 1000: t += (min(g, 4000) - 1000) * Fraction(1, 5)
    if g > 4000: t += (g - 4000) * Fraction(2, 5)
    return rnd(t)

emps = {}
hours = {}
bonus = {}
out = []
for n, raw in enumerate(sys.stdin.read().split('\n'), 1):
    t = raw.split()
    if not t: continue
    def bad(): out.append("line %d: bad line" % n)
    c = t[0]
    if c == 'EMP' and len(t) == 5 and t[3] in ('hourly', 'salary'):
        v = dec(t[4])
        if v is None: bad(); continue
        if t[1] in emps: out.append("line %d: duplicate ID" % n); continue
        emps[t[1]] = (t[2], t[3], v)
    elif c == 'HOURS' and len(t) == 4:
        d = date(t[2]); h = dec(t[3])
        if d is None or h is None: bad(); continue
        if t[1] not in emps: out.append("line %d: unknown ID" % n); continue
        if emps[t[1]][1] != 'hourly': out.append("line %d: not hourly" % n); continue
        k = (t[1], d)
        if hours.get(k, 0) + h > 24: out.append("line %d: too many hours" % n); continue
        hours[k] = hours.get(k, 0) + h
    elif c == 'BONUS' and len(t) == 4:
        a = dec(t[3])
        if not MON.match(t[2]) or a is None: bad(); continue
        if t[1] not in emps: out.append("line %d: unknown ID" % n); continue
        bonus[(t[1], t[2])] = bonus.get((t[1], t[2]), 0) + a
    elif c == 'RUN' and len(t) == 2 and MON.match(t[1]):
        m = t[1]
        out.append("payroll " + m)
        tg = tt = tn = 0
        for i in sorted(emps):
            name, kind, r = emps[i]
            if kind == 'salary':
                pay = r / 12
            else:
                pay = Fraction(0)
                for (j, d), h in hours.items():
                    if j != i or d.strftime('%Y-%m') != m: continue
                    if d.weekday() >= 5:
                        pay += h * r * 2
                    else:
                        pay += min(h, 8) * r + max(h - 8, 0) * r * Fraction(3, 2)
            pay += bonus.get((i, m), 0)
            g = rnd(pay); x = tax(g); nt = g - x
            tg += g; tt += x; tn += nt
            out.append("  %s %s gross %s tax %s net %s" % (i, name, fmt(g), fmt(x), fmt(nt)))
        out.append("  total gross %s tax %s net %s" % (fmt(tg), fmt(tt), fmt(tn)))
    else:
        bad()
print('\n'.join(out))
