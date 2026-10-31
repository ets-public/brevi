import datetime
from catalog import TIERS, Book, Member, fmt, cents


def add_book(lib, bid, copies, title):
    if bid in lib.books:
        return ["duplicate ID"]
    lib.books[bid] = Book(bid, copies, title)
    return ["added " + bid]


def add_member(lib, mid, tier, name):
    if mid in lib.members:
        return ["duplicate ID"]
    lib.members[mid] = Member(mid, tier, name)
    return ["welcome " + name]


def give_copy(book, day):
    """A copy becomes free on `day`: hold it for the first waiting member or shelve it."""
    if book.waiting:
        m = book.waiting.pop(0)
        book.holds.append([m, day + 3])
    else:
        book.shelf += 1


def expire_holds(lib, today):
    out = []
    while True:
        best = None
        for b in lib.books.values():
            for h in b.holds:
                if h[1] < today and (best is None or (h[1], b.id) < (best[0][1], best[1].id)):
                    best = (h, b)
        if best is None:
            return out
        h, b = best
        b.holds.remove(h)
        out.append("hold expired %s %s" % (h[0], b.id))
        give_copy(b, h[1])


def loan(lib, day, mid, bid):
    m = lib.members.get(mid)
    if m is None:
        return ["refused %s %s: unknown member" % (mid, bid)]
    b = lib.books.get(bid)
    if b is None:
        return ["refused %s %s: unknown book" % (mid, bid)]
    t = TIERS[m.tier]
    if m.owes > 200:
        reason = "owes " + fmt(m.owes)
    elif len(m.loans) >= t['max']:
        reason = "limit"
    elif any(l[0] == bid for l in m.loans):
        reason = "already has it"
    else:
        hold = None
        for h in b.holds:
            if h[0] == mid:
                hold = h
                break
        if hold is not None:
            b.holds.remove(hold)
        elif b.shelf > 0:
            b.shelf -= 1
        else:
            return ["refused %s %s: none available" % (mid, bid)]
        due = day + t['days']
        m.loans.append([bid, due])
        return ["loan %s %s due %s" % (mid, bid, datetime.date.fromordinal(due).isoformat())]
    return ["refused %s %s: %s" % (mid, bid, reason)]


def give_back(lib, day, mid, bid):
    m = lib.members.get(mid)
    loan_rec = None
    if m is not None:
        for l in m.loans:
            if l[0] == bid:
                loan_rec = l
                break
    if loan_rec is None:
        return ["refused %s %s: not on loan" % (mid, bid)]
    m.loans.remove(loan_rec)
    fee = 0
    if day > loan_rec[1]:
        fee = min(500, (day - loan_rec[1]) * TIERS[m.tier]['fee'])
        m.owes += fee
    give_copy(lib.books[bid], day)
    s = "returned %s %s" % (mid, bid)
    if fee:
        s += " fee " + fmt(fee)
    return [s]


def reserve(lib, mid, bid):
    m = lib.members.get(mid)
    if m is None:
        return ["refused %s %s: unknown member" % (mid, bid)]
    b = lib.books.get(bid)
    if b is None:
        return ["refused %s %s: unknown book" % (mid, bid)]
    if any(l[0] == bid for l in m.loans):
        return ["refused %s %s: already has it" % (mid, bid)]
    if mid in b.waiting or any(h[0] == mid for h in b.holds):
        return ["refused %s %s: already reserved" % (mid, bid)]
    if b.shelf > 0:
        return ["refused %s %s: copy available" % (mid, bid)]
    b.waiting.append(mid)
    return ["reserved %s %s position %d" % (mid, bid, len(b.waiting))]


def pay(lib, mid, amount):
    m = lib.members.get(mid)
    if m is None:
        return ["refused %s: unknown member" % mid]
    m.owes -= cents(amount)
    return ["paid %s owes %s" % (mid, fmt(m.owes))]


def status(lib, mid):
    m = lib.members.get(mid)
    if m is None:
        return ["refused %s: unknown member" % mid]
    ls = ' '.join(l[0] for l in m.loans) or '-'
    return ["%s %s owes %s loans: %s" % (mid, m.name, fmt(m.owes), ls)]
