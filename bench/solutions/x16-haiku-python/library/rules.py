"""Lending rules: loans, returns, fees, reservations and holds."""
from datetime import timedelta

from catalog import TIERS, MAX_FEE, HOLD_DAYS, Loan, Hold, money


def refused(mid, bid, reason):
    return 'refused %s %s: %s' % (mid, bid, reason)


def find_hold(lib, mid, bid):
    for h in lib.holds:
        if h.member_id == mid and h.book_id == bid:
            return h
    return None


def expire_holds(lib, today):
    """Expire every hold whose last day is before today; pass copies on."""
    out = []
    while True:
        late = [h for h in lib.holds if h.last < today]
        if not late:
            break
        h = min(late, key=lambda x: (x.last, x.book_id))
        lib.holds.remove(h)
        out.append('hold expired %s %s' % (h.member_id, h.book_id))
        book = lib.books[h.book_id]
        if book.waiting:
            nxt = book.waiting.pop(0)
            # new hold starts the day after the old one ended and covers HOLD_DAYS days
            lib.holds.append(Hold(nxt, h.book_id, h.last + timedelta(days=HOLD_DAYS)))
        else:
            book.shelf += 1
    return out


def loan(lib, today, mid, bid):
    m = lib.members.get(mid)
    if m is None:
        return refused(mid, bid, 'unknown member')
    b = lib.books.get(bid)
    if b is None:
        return refused(mid, bid, 'unknown book')
    if m.owed > 200:
        return refused(mid, bid, 'owes ' + money(m.owed))
    tier = TIERS[m.tier]
    if len(m.loans) >= tier['limit']:
        return refused(mid, bid, 'limit')
    if any(l.book_id == bid for l in m.loans):
        return refused(mid, bid, 'already has it')
    hold = find_hold(lib, mid, bid)
    if hold is None and b.shelf == 0:
        return refused(mid, bid, 'none available')
    if hold is not None:
        lib.holds.remove(hold)
    else:
        b.shelf -= 1
    due = today + timedelta(days=tier['days'])
    m.loans.append(Loan(bid, due))
    return 'loan %s %s due %s' % (mid, bid, due.isoformat())


def ret(lib, today, mid, bid):
    m = lib.members.get(mid)
    found = None
    if m is not None:
        for l in m.loans:
            if l.book_id == bid:
                found = l
                break
    if found is None:
        return refused(mid, bid, 'not on loan')
    m.loans.remove(found)
    tier = TIERS[m.tier]
    fee = 0
    if today > found.due:
        days = (today - found.due).days
        fee = min(days * tier['fee'], MAX_FEE)
        m.owed += fee
    msg = 'returned %s %s' % (mid, bid)
    if fee:
        msg += ' fee %s' % money(fee)
    book = lib.books[bid]
    if book.waiting:
        nxt = book.waiting.pop(0)
        lib.holds.append(Hold(nxt, bid, today + timedelta(days=HOLD_DAYS)))
    else:
        book.shelf += 1
    return msg


def reserve(lib, today, mid, bid):
    m = lib.members.get(mid)
    if m is None:
        return refused(mid, bid, 'unknown member')
    b = lib.books.get(bid)
    if b is None:
        return refused(mid, bid, 'unknown book')
    if any(l.book_id == bid for l in m.loans):
        return refused(mid, bid, 'already has it')
    if mid in b.waiting or find_hold(lib, mid, bid) is not None:
        return refused(mid, bid, 'already reserved')
    if b.shelf > 0:
        return refused(mid, bid, 'copy available')
    b.waiting.append(mid)
    return 'reserved %s %s position %d' % (mid, bid, len(b.waiting))


def pay(lib, mid, amount):
    m = lib.members.get(mid)
    if m is None:
        return 'refused %s: unknown member' % mid
    m.owed -= amount
    return 'paid %s owes %s' % (mid, money(m.owed))


def status(lib, mid):
    m = lib.members.get(mid)
    if m is None:
        return 'refused %s: unknown member' % mid
    books = ' '.join(l.book_id for l in m.loans) or '-'
    return '%s %s owes %s loans: %s' % (mid, m.name, money(m.owed), books)
