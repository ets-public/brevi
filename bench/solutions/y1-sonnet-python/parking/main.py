import sys
import re
import datetime
from garage import Garage
from billing import fee_cents, stay_text, money


def stamp(d, t):
    m = re.fullmatch(r'(\d{4})-(\d{2})-(\d{2})', d)
    k = re.fullmatch(r'(\d{2}):(\d{2})', t)
    if not m or not k:
        return None
    try:
        dt = datetime.datetime(int(m.group(1)), int(m.group(2)), int(m.group(3)), int(k.group(1)), int(k.group(2)))
    except ValueError:
        return None
    return int((dt - datetime.datetime(1970, 1, 1)).total_seconds() // 60)


def main():
    lines = sys.stdin.read().split('\n')
    h = lines[0].split()
    g = Garage(int(h[2]), int(h[4]), int(h[6]))
    out = []
    revenue = 0
    exits = 0
    prev = None
    for n in range(2, len(lines) + 1):
        raw = lines[n - 1]
        if not raw.strip():
            continue
        t = raw.split()
        cmd = t[2:] if len(t) >= 3 else None
        when = stamp(t[0], t[1]) if cmd else None
        ok = when is not None and (
            (cmd[0] == 'IN' and len(cmd) == 3 and cmd[2] in ('car', 'moto', 'van'))
            or (cmd[0] == 'OUT' and len(cmd) == 2)
            or (cmd[0] == 'REPORT' and len(cmd) == 1))
        if not ok:
            out.append('line %d: bad line' % n)
            continue
        if prev is not None and when < prev:
            out.append('line %d: time goes back' % n)
            continue
        prev = when
        if cmd[0] == 'IN':
            if cmd[1] in g.inside:
                out.append('line %d: already inside' % n)
                continue
            sp = g.enter(cmd[1], cmd[2], when)
            out.append('IN %s %s' % (cmd[1], sp) if sp else 'FULL %s' % cmd[1])
        elif cmd[0] == 'OUT':
            if cmd[1] not in g.inside:
                out.append('line %d: not inside' % n)
                continue
            kind, sp, arr = g.leave(cmd[1])
            d = when - arr
            f = fee_cents(kind, d)
            revenue += f
            exits += 1
            out.append('OUT %s %s fee %s' % (cmd[1], stay_text(d), money(f)))
        else:
            out.append('occupied car %d/%d moto %d/%d van %d/%d revenue %s' % (
                g.count('C'), g.cap['C'], g.count('M'), g.cap['M'], g.count('V'), g.cap['V'], money(revenue)))
    out.append('revenue %s from %d exits' % (money(revenue), exits))
    print('\n'.join(out))


main()
