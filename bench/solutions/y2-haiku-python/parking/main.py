import sys
import re
import datetime

from garage import Garage
from billing import fee_cents, fmt_money, fmt_stay

DATE_RE = re.compile(r'[0-9]{4}-[0-9]{2}-[0-9]{2}')
TIME_RE = re.compile(r'([0-9]{2}):([0-9]{2})')
TYPES = ('car', 'moto', 'van')


def stamp_of(ds, ts):
    """Minutes since the epoch for a valid date and time, else None."""
    if not DATE_RE.fullmatch(ds):
        return None
    tm = TIME_RE.fullmatch(ts)
    if not tm:
        return None
    hh, mm = int(tm.group(1)), int(tm.group(2))
    if hh > 23 or mm > 59:
        return None
    try:
        d = datetime.date.fromisoformat(ds)
    except ValueError:
        return None
    return d.toordinal() * 1440 + hh * 60 + mm


def main():
    lines = sys.stdin.read().splitlines()
    head = lines[0].split() if lines else []
    cars, motos, vans = int(head[2]), int(head[4]), int(head[6])
    g = Garage(cars, motos, vans)
    clock = None
    revenue = 0
    exits = 0

    for n, raw in enumerate(lines[1:], 2):
        p = raw.split()
        if not p:
            continue
        stamp = stamp_of(p[0], p[1]) if len(p) >= 2 else None
        cmd = p[2] if len(p) >= 3 else ''
        kind = None
        if stamp is not None:
            if cmd == 'IN' and len(p) == 5 and p[4] in TYPES:
                kind = 'IN'
            elif cmd == 'OUT' and len(p) == 4:
                kind = 'OUT'
            elif cmd == 'REPORT' and len(p) == 3:
                kind = 'REPORT'
        if kind is None:
            print(f"line {n}: bad line")
            continue
        if clock is not None and stamp < clock:
            print(f"line {n}: time goes back")
            continue

        if kind == 'IN':
            plate, vtype = p[3], p[4]
            if g.inside(plate):
                print(f"line {n}: already inside")
                continue
            space = g.enter(plate, vtype, stamp)
            if space is None:
                print(f"FULL {plate}")
            else:
                print(f"IN {plate} {space}")
        elif kind == 'OUT':
            plate = p[3]
            if not g.inside(plate):
                print(f"line {n}: not inside")
                continue
            vtype, t_in = g.leave(plate)
            stay = stamp - t_in
            fee = fee_cents(vtype, stay)
            revenue += fee
            exits += 1
            print(f"OUT {plate} {fmt_stay(stay)} fee {fmt_money(fee)}")
        else:
            c = g.counts()
            print(f"occupied car {c['C'][0]}/{c['C'][1]} "
                  f"moto {c['M'][0]}/{c['M'][1]} "
                  f"van {c['V'][0]}/{c['V'][1]} "
                  f"revenue {fmt_money(revenue)}")
        clock = stamp

    print(f"revenue {fmt_money(revenue)} from {exits} exits")


if __name__ == '__main__':
    main()
