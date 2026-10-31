import datetime

from catalog import TIERS, Hold

MAX_FEE = 500      # cents per loan
HOLD_DAYS = 3      # a hold from day D can be collected through D + HOLD_DAYS
OWES_LIMIT = 200   # cents


def money(c):
    sign = '-' if c < 0 else ''
    c = abs(c)
    return "%s%d.%02d" % (sign, c // 100, c % 100)


def to_cents(s):
    if '.' in s:
        a, b = s.split('.')
        b = (b + '00')[:2]
    else:
        a, b = s, '00'
    return int(a) * 100 + int(b)


def refuse(mid, bid, reason):
    return "refused %s %s: %s" % (mid, bid, reason)


def find_hold(book, mid):
    for h in book.holds:
        if h.member_id == mid:
            return h
    return None


def expire_holds(lib, date):
    """Expire every hold whose last day is before date, in (last day, book ID) order."""
    out = []
    while True:
        best = None
        for b in lib.books.values():
            for h in b.holds:
                if h.last < date:
                    key = (h.last, b.id)
                    if best is None or key < best[0]:
                        best = (key, b, h)
        if best is None:
            break
        _key, b, h = best
        b.holds.remove(h)
        out.append("hold expired %s %s" % (h.member_id, b.id))
        if b.waitlist:
            nxt = b.waitlist.pop(0)
            b.holds.append(Hold(nxt, h.last + datetime.timedelta(days=HOLD_DAYS)))
        else:
            b.shelf += 1
    return out


def loan(lib, date, mid, bid):
    m = lib.members.get(mid)
    if m is None:
        return refuse(mid, bid, 'unknown member')
    b = lib.books.get(bid)
    if b is None:
        return refuse(mid, bid, 'unknown book')
    if m.owes > OWES_LIMIT:
        return refuse(mid, bid, 'owes %s' % money(m.owes))
    t = TIERS[m.tier]
    if len(m.loans) >= t['limit']:
        return refuse(mid, bid, 'limit')
    if any(lb == bid for lb, _due in m.loans):
        return refuse(mid, bid, 'already has it')
    hold = find_hold(b, mid)
    if hold is None and b.shelf <= 0:
        return refuse(mid, bid, 'none available')
    due = date + datetime.timedelta(days=t['days'])
    if hold is not None:
        b.holds.remove(hold)
    else:
        b.shelf -= 1
    m.loans.append((bid, due))
    return "loan %s %s due %s" % (mid, bid, due.isoformat())


def return_book(lib, date, mid, bid):
    m = lib.members.get(mid)
    idx = None
    if m is not None:
        for i, (lb, _due) in enumerate(m.loans):
            if lb == bid:
                idx = i
                break
    if idx is None:
        return refuse(mid, bid, 'not on loan')
    _lb, due = m.loans.pop(idx)
    out = "returned %s %s" % (mid, bid)
    late = (date - due).days
    if late > 0:
        fee = min(late * TIERS[m.tier]['rate'], MAX_FEE)
        m.owes += fee
        out += " fee %s" % money(fee)
    b = lib.books[bid]
    if b.waitlist:
        nxt = b.waitlist.pop(0)
        b.holds.append(Hold(nxt, date + datetime.timedelta(days=HOLD_DAYS)))
    else:
        b.shelf += 1
    return out


def reserve(lib, date, mid, bid):
    m = lib.members.get(mid)
    if m is None:
        return refuse(mid, bid, 'unknown member')
    b = lib.books.get(bid)
    if b is None:
        return refuse(mid, bid, 'unknown book')
    if any(lb == bid for lb, _due in m.loans):
        return refuse(mid, bid, 'already has it')
    if mid in b.waitlist or find_hold(b, mid) is not None:
        return refuse(mid, bid, 'already reserved')
    if b.shelf > 0:
        return refuse(mid, bid, 'copy available')
    b.waitlist.append(mid)
    return "reserved %s %s position %d" % (mid, bid, len(b.waitlist))


def pay(lib, mid, amount):
    m = lib.members.get(mid)
    if m is None:
        return "refused %s: unknown member" % mid
    m.owes -= amount
    return "paid %s owes %s" % (mid, money(m.owes))


def status(lib, mid):
    m = lib.members.get(mid)
    if m is None:
        return "refused %s: unknown member" % mid
    loans = ' '.join(lb for lb, _due in m.loans) or '-'
    return "%s %s owes %s loans: %s" % (mid, m.name, money(m.owes), loans)
