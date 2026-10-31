import sys, re, datetime
from garage import Garage
from billing import stay_minutes, fee_cents, fmt_stay, money

def parse_when(d, t):
    m1 = re.fullmatch(r'(\d{4})-(\d\d)-(\d\d)', d)
    m2 = re.fullmatch(r'(\d\d):(\d\d)', t)
    if not m1 or not m2:
        return None
    try:
        return datetime.datetime(int(m1.group(1)), int(m1.group(2)), int(m1.group(3)), int(m2.group(1)), int(m2.group(2)))
    except ValueError:
        return None

def main():
    lines = sys.stdin.read().split('\n')
    first = lines[0].split()
    C, M, V = int(first[2]), int(first[4]), int(first[6])
    g = Garage(C, M, V)
    clock = None
    revenue = 0
    exits = 0
    for n, raw in enumerate(lines[1:], 2):
        t = raw.split()
        if not t:
            continue
        when = parse_when(t[0], t[1]) if len(t) >= 3 else None
        cmd = t[2] if len(t) >= 3 else None
        args = t[3:]
        ok = when is not None and (
            (cmd == 'IN' and len(args) == 2 and args[1] in ('car', 'moto', 'van')) or
            (cmd == 'OUT' and len(args) == 1) or
            (cmd == 'REPORT' and not args))
        if not ok:
            print('line %d: bad line' % n)
            continue
        if clock is not None and when < clock:
            print('line %d: time goes back' % n)
            continue
        if cmd == 'IN':
            plate, kind = args
            if plate in g.inside:
                print('line %d: already inside' % n)
                continue
            sp = g.enter(plate, kind, when)
            print('FULL %s' % plate if sp is None else 'IN %s %s' % (plate, sp))
        elif cmd == 'OUT':
            plate = args[0]
            if plate not in g.inside:
                print('line %d: not inside' % n)
                continue
            kind, t_in = g.leave(plate)
            mins = stay_minutes(t_in, when)
            fee = fee_cents(kind, mins)
            revenue += fee
            exits += 1
            print('OUT %s %s fee %s' % (plate, fmt_stay(mins), money(fee)))
        else:
            c = g.counts()
            print('occupied car %d/%d moto %d/%d van %d/%d revenue %s' % (c['C'], C, c['M'], M, c['V'], V, money(revenue)))
        clock = when
    print('revenue %s from %d exits' % (money(revenue), exits))

main()
