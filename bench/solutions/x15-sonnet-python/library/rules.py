import datetime

TIERS = {"basic": (2, 14, 25), "plus": (5, 28, 10)}


def fmt(c):
    sign = "-" if c < 0 else ""
    c = abs(c)
    return "%s%d.%02d" % (sign, c // 100, c % 100)


def parse_date(s):
    return datetime.date(int(s[:4]), int(s[5:7]), int(s[8:10]))


def expire_holds(lib, today):
    out = []
    while True:
        cand = [h for h in lib.holds if h["last"] < today]
        if not cand:
            return out
        h = min(cand, key=lambda x: (x["last"], x["book"]))
        lib.holds.remove(h)
        out.append("hold expired %s %s" % (h["member"], h["book"]))
        book = lib.books[h["book"]]
        if book.waiting:
            m = book.waiting.pop(0)
            lib.holds.append({"member": m, "book": book.id,
                              "last": h["last"] + datetime.timedelta(days=3)})
        else:
            book.shelf += 1


def has_hold(lib, mid, bid):
    for h in lib.holds:
        if h["member"] == mid and h["book"] == bid:
            return h
    return None


def loan(lib, today, mid, bid):
    m = lib.members.get(mid)
    if m is None:
        return "refused %s %s: unknown member" % (mid, bid)
    b = lib.books.get(bid)
    if b is None:
        return "refused %s %s: unknown book" % (mid, bid)
    limit, days, _ = TIERS[m.tier]
    if m.owes > 200:
        return "refused %s %s: owes %s" % (mid, bid, fmt(m.owes))
    if len(m.loans) >= limit:
        return "refused %s %s: limit" % (mid, bid)
    if any(l[0] == bid for l in m.loans):
        return "refused %s %s: already has it" % (mid, bid)
    h = has_hold(lib, mid, bid)
    if h is not None:
        lib.holds.remove(h)
    elif b.shelf > 0:
        b.shelf -= 1
    else:
        return "refused %s %s: none available" % (mid, bid)
    due = today + datetime.timedelta(days=days)
    m.loans.append([bid, due])
    return "loan %s %s due %s" % (mid, bid, due.isoformat())


def give_back(lib, today, mid, bid):
    m = lib.members.get(mid)
    b = lib.books.get(bid)
    ln = None
    if m is not None:
        for l in m.loans:
            if l[0] == bid:
                ln = l
                break
    if ln is None:
        return "refused %s %s: not on loan" % (mid, bid)
    m.loans.remove(ln)
    _, _, daily = TIERS[m.tier]
    fee = 0
    if today > ln[1]:
        fee = min((today - ln[1]).days * daily, 500)
    m.owes += fee
    if b.waiting:
        nxt = b.waiting.pop(0)
        lib.holds.append({"member": nxt, "book": bid,
                          "last": today + datetime.timedelta(days=3)})
    else:
        b.shelf += 1
    s = "returned %s %s" % (mid, bid)
    if fee:
        s += " fee " + fmt(fee)
    return s


def reserve(lib, today, mid, bid):
    m = lib.members.get(mid)
    if m is None:
        return "refused %s %s: unknown member" % (mid, bid)
    b = lib.books.get(bid)
    if b is None:
        return "refused %s %s: unknown book" % (mid, bid)
    if any(l[0] == bid for l in m.loans):
        return "refused %s %s: already has it" % (mid, bid)
    if mid in b.waiting or has_hold(lib, mid, bid):
        return "refused %s %s: already reserved" % (mid, bid)
    if b.shelf > 0:
        return "refused %s %s: copy available" % (mid, bid)
    b.waiting.append(mid)
    return "reserved %s %s position %d" % (mid, bid, len(b.waiting))


def pay(lib, mid, amount):
    m = lib.members.get(mid)
    if m is None:
        return "refused %s: unknown member" % mid
    m.owes -= amount
    return "paid %s owes %s" % (mid, fmt(m.owes))


def status(lib, mid):
    m = lib.members.get(mid)
    if m is None:
        return "refused %s: unknown member" % mid
    loans = " ".join(l[0] for l in m.loans) or "-"
    return "%s %s owes %s loans: %s" % (mid, m.name, fmt(m.owes), loans)
