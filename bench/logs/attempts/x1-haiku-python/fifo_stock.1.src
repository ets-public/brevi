import datetime
import re
import sys

DATE_RE = re.compile(r'[0-9]{4}-[0-9]{2}-[0-9]{2}')
QTY_RE = re.compile(r'[0-9]+')
AMT_RE = re.compile(r'[0-9]+(\.[0-9]{1,2})?')


def to_cents(s):
    if '.' in s:
        a, b = s.split('.')
        b = (b + '00')[:2]
    else:
        a, b = s, '00'
    return int(a) * 100 + int(b)


def money(c):
    sign = '-' if c < 0 else ''
    c = abs(c)
    return "%s%d.%02d" % (sign, c // 100, c % 100)


def valid_date(s):
    if not DATE_RE.fullmatch(s):
        return False
    try:
        datetime.date.fromisoformat(s)
    except ValueError:
        return False
    return True


def main():
    lines = sys.stdin.read().split('\n')
    out_err = []
    lots = {}      # sku -> list of [qty, unit_cost_cents], oldest first
    stats = {}     # sku -> dict
    months = {}    # 'YYYY-MM' -> {'profit': c, 'loss': c}
    latest = None

    def stat(sku):
        if sku not in stats:
            stats[sku] = {'cogs': 0, 'rev': 0, 'profit': 0, 'loss': 0}
        return stats[sku]

    def on_hand(sku):
        return sum(l[0] for l in lots.get(sku, []))

    def take(sku, qty):
        cost = 0
        need = qty
        q = lots.get(sku, [])
        while need > 0:
            lot = q[0]
            t = min(need, lot[0])
            cost += t * lot[1]
            lot[0] -= t
            need -= t
            if lot[0] == 0:
                q.pop(0)
        lots[sku] = q
        return cost

    for lineno, raw in enumerate(lines, 1):
        line = raw.strip()
        if not line:
            continue
        tok = line.split()
        ok_form = False
        if len(tok) >= 2 and valid_date(tok[0]):
            op = tok[1]
            if op == 'RECEIVE' and len(tok) == 5:
                ok_form = bool(QTY_RE.fullmatch(tok[3]) and int(tok[3]) >= 1 and AMT_RE.fullmatch(tok[4]))
            elif op == 'SHIP' and len(tok) == 5:
                ok_form = bool(QTY_RE.fullmatch(tok[3]) and int(tok[3]) >= 1 and AMT_RE.fullmatch(tok[4]))
            elif op == 'LOSS' and len(tok) == 4:
                ok_form = bool(QTY_RE.fullmatch(tok[3]) and int(tok[3]) >= 1)
        if not ok_form:
            out_err.append("line %d: bad line" % lineno)
            continue

        date, op, sku = tok[0], tok[1], tok[2]
        qty = int(tok[3])
        if latest is not None and date < latest:
            out_err.append("line %d: date out of order" % lineno)
            continue

        if op == 'RECEIVE':
            cost = to_cents(tok[4])
            latest = date
            stat(sku)
            lots.setdefault(sku, []).append([qty, cost])
            continue

        have = on_hand(sku)
        if qty > have:
            out_err.append("line %d: short %s need %d have %d" % (lineno, sku, qty, have))
            continue

        latest = date
        mon = date[:7]
        st = stat(sku)
        cost = take(sku, qty)
        if op == 'SHIP':
            price = to_cents(tok[4])
            rev = qty * price
            profit = rev - cost
            st['cogs'] += cost
            st['rev'] += rev
            st['profit'] += profit
            m = months.setdefault(mon, {'profit': 0, 'loss': 0})
            m['profit'] += profit
        else:
            st['loss'] += cost
            m = months.setdefault(mon, {'profit': 0, 'loss': 0})
            m['loss'] += cost

    out = list(out_err)
    for sku in sorted(stats):
        st = stats[sku]
        q = on_hand(sku)
        value = sum(l[0] * l[1] for l in lots.get(sku, []))
        out.append("%s on_hand %d value %s cogs %s revenue %s profit %s loss %s" % (
            sku, q, money(value), money(st['cogs']), money(st['rev']),
            money(st['profit']), money(st['loss'])))
    out.append('months')
    for mon in sorted(months):
        m = months[mon]
        out.append("  %s profit %s loss %s" % (mon, money(m['profit']), money(m['loss'])))
    tp = sum(s['profit'] for s in stats.values())
    tl = sum(s['loss'] for s in stats.values())
    out.append("total profit %s loss %s net %s" % (money(tp), money(tl), money(tp - tl)))
    sys.stdout.write('\n'.join(out) + '\n')


main()
