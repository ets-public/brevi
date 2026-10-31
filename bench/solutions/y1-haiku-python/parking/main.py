import datetime
import re
import sys

from garage import Garage, KIND
from billing import fee_cents, money, stay_text

DATE_RE = re.compile(r'[0-9]{4}-[0-9]{2}-[0-9]{2}')
TIME_RE = re.compile(r'[0-9]{2}:[0-9]{2}')
INT_RE = re.compile(r'0|[1-9][0-9]*')


def parse_when(date_s, time_s):
    if not DATE_RE.fullmatch(date_s) or not TIME_RE.fullmatch(time_s):
        return None
    try:
        return datetime.datetime(
            int(date_s[0:4]), int(date_s[5:7]), int(date_s[8:10]),
            int(time_s[0:2]), int(time_s[3:5]))
    except ValueError:
        return None


def main():
    lines = sys.stdin.read().split('\n')
    cfg = lines[0].split() if lines else []
    if (len(cfg) != 7 or cfg[0] != 'spaces' or cfg[1] != 'car' or cfg[3] != 'moto'
            or cfg[5] != 'van' or not all(INT_RE.fullmatch(x) for x in (cfg[2], cfg[4], cfg[6]))):
        return
    garage = Garage(int(cfg[2]), int(cfg[4]), int(cfg[6]))

    entered = {}  # plate -> entry datetime
    revenue = 0
    exits = 0
    last = None
    out = []

    for lineno, raw in enumerate(lines[1:], 2):
        tok = raw.strip().split()
        if not tok:
            continue

        def bad():
            out.append('line %d: bad line' % lineno)

        when = parse_when(tok[0], tok[1]) if len(tok) >= 3 else None
        if when is None:
            bad()
            continue
        cmd = tok[2:]
        if cmd[0] == 'IN' and len(cmd) == 3 and cmd[2] in KIND:
            plate, vtype = cmd[1], cmd[2]
            if last is not None and when < last:
                out.append('line %d: time goes back' % lineno)
                continue
            if plate in garage.inside:
                out.append('line %d: already inside' % lineno)
                continue
            last = when
            space = garage.allocate(vtype, plate)
            if space is None:
                out.append('FULL %s' % plate)
            else:
                entered[plate] = when
                out.append('IN %s %s' % (plate, space))
        elif cmd[0] == 'OUT' and len(cmd) == 2:
            plate = cmd[1]
            if last is not None and when < last:
                out.append('line %d: time goes back' % lineno)
                continue
            if plate not in garage.inside:
                out.append('line %d: not inside' % lineno)
                continue
            last = when
            vtype, _ = garage.inside[plate]
            minutes = int((when - entered.pop(plate)).total_seconds() // 60)
            fee = fee_cents(vtype, minutes)
            garage.release(plate)
            revenue += fee
            exits += 1
            out.append('OUT %s %s fee %s' % (plate, stay_text(minutes), money(fee)))
        elif cmd == ['REPORT']:
            if last is not None and when < last:
                out.append('line %d: time goes back' % lineno)
                continue
            last = when
            out.append('occupied car %d/%d moto %d/%d van %d/%d revenue %s' % (
                garage.occupied('C'), garage.total('C'),
                garage.occupied('M'), garage.total('M'),
                garage.occupied('V'), garage.total('V'),
                money(revenue)))
        else:
            bad()

    out.append('revenue %s from %d exits' % (money(revenue), exits))
    print('\n'.join(out))


if __name__ == '__main__':
    main()
