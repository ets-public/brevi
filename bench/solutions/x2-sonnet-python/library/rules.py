import datetime

TIERS = {"basic": (2, 14, 25), "plus": (5, 28, 10)}


def money(c):
    s = "-" if c < 0 else ""
    c = abs(c)
    return "%s%d.%02d" % (s, c // 100, c % 100)


def parse_money(s):
    try:
        a, _, b = s.partition('.')
        return int(a) * 100 + int(b.ljust(2, '0')[:2]) if b else int(a) * 100
    except ValueError:
        return 0


def to_date(s):
    return datetime.date(int(s[:4]), int(s[5:7]), int(s[8:10]))


def expire_holds(lib, today):
    out = []
    while True:
        due = [h for h in lib.holds if h[0] < today]
        if not due:
            return out
        h = min(due, key=lambda x: (x[0], x[1]))
        lib.holds.remove(h)
        last, bid, mid = h
        out.append("hold expired %s %s" % (mid, bid))
        book = lib.books[bid]
        if book.waiting:
            nxt = book.waiting.pop(0)
            nl = to_date(last) + datetime.timedelta(days=3)
            lib.holds.append([nl.isoformat(), bid, nxt])
        else:
            book.shelf += 1


def find_hold(lib, mid, bid):
    for h in lib.holds:
        if h[1] == bid and h[2] == mid:
            return h
    return None


def loan(lib, date, mid, bid):
    m = lib.members.get(mid)
    if m is None:
        return "refused %s %s: unknown member" % (mid, bid)
    b = lib.books.get(bid)
    if b is None:
        return "refused %s %s: unknown book" % (mid, bid)
    mx, days, _ = TIERS[m.tier]
    if m.owes > 200:
        return "refused %s %s: owes %s" % (mid, bid, money(m.owes))
    if len(m.loans) >= mx:
        return "refused %s %s: limit" % (mid, bid)
    if any(l[0] == bid for l in m.loans):
        return "refused %s %s: already has it" % (mid, bid)
    h = find_hold(lib, mid, bid)
    if h is not None:
        lib.holds.remove(h)
    elif b.shelf > 0:
        b.shelf -= 1
    else:
        return "refused %s %s: none available" % (mid, bid)
    due = (to_date(date) + datetime.timedelta(days=days)).isoformat()
    m.loans.append([bid, due])
    return "loan %s %s due %s" % (mid, bid, due)


def give_back(lib, date, mid, bid):
    m = lib.members.get(mid)
    rec = None
    if m is not None:
        for l in m.loans:
            if l[0] == bid:
                rec = l
    if rec is None:
        return "refused %s %s: not on loan" % (mid, bid)
    m.loans.remove(rec)
    _, _, rate = TIERS[m.tier]
    late = (to_date(date) - to_date(rec[1])).days
    fee = min(500, late * rate) if late > 0 else 0
    m.owes += fee
    b = lib.books[bid]
    if b.waiting:
        nxt = b.waiting.pop(0)
        last = to_date(date) + datetime.timedelta(days=3)
        lib.holds.append([last.isoformat(), bid, nxt])
    else:
        b.shelf += 1
    s = "returned %s %s" % (mid, bid)
    if fee:
        s += " fee " + money(fee)
    return s


def reserve(lib, date, mid, bid):
    m = lib.members.get(mid)
    if m is None:
        return "refused %s %s: unknown member" % (mid, bid)
    b = lib.books.get(bid)
    if b is None:
        return "refused %s %s: unknown book" % (mid, bid)
    if any(l[0] == bid for l in m.loans):
        return "refused %s %s: already has it" % (mid, bid)
    if mid in b.waiting or find_hold(lib, mid, bid) is not None:
        return "refused %s %s: already reserved" % (mid, bid)
    if b.shelf > 0:
        return "refused %s %s: copy available" % (mid, bid)
    b.waiting.append(mid)
    return "reserved %s %s position %d" % (mid, bid, len(b.waiting))


def pay(lib, mid, amount):
    m = lib.members.get(mid)
    if m is None:
        return "refused %s: unknown member" % mid
    m.owes -= parse_money(amount)
    return "paid %s owes %s" % (mid, money(m.owes))


def status(lib, mid):
    m = lib.members.get(mid)
    if m is None:
        return "refused %s: unknown member" % mid
    ls = " ".join(l[0] for l in m.loans) or "-"
    return "%s %s owes %s loans: %s" % (mid, m.name, money(m.owes), ls)
