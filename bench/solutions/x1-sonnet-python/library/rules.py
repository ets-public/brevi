from catalog import from_ord, money


def expire_holds(lib, today):
    out = []
    while True:
        due = [h for h in lib.holds if h[0] < today]
        if not due:
            return out
        h = min(due, key=lambda x: (x[0], x[1]))
        lib.holds.remove(h)
        out.append("hold expired %s %s" % (h[2], h[1]))
        book = lib.books[h[1]]
        if book.waiting:
            nxt = book.waiting.pop(0)
            lib.holds.append([h[0] + 3, h[1], nxt])
        else:
            book.shelf += 1


def has_book(m, bid):
    return any(l[0] == bid for l in m.loans)


def loan(lib, today, mid, bid):
    m = lib.members.get(mid)
    if m is None:
        return "refused %s %s: unknown member" % (mid, bid)
    b = lib.books.get(bid)
    if b is None:
        return "refused %s %s: unknown book" % (mid, bid)
    if m.owes > 200:
        return "refused %s %s: owes %s" % (mid, bid, money(m.owes))
    if len(m.loans) >= m.limit:
        return "refused %s %s: limit" % (mid, bid)
    if has_book(m, bid):
        return "refused %s %s: already has it" % (mid, bid)
    hold = None
    for h in lib.holds:
        if h[1] == bid and h[2] == mid:
            hold = h
            break
    if hold is not None:
        lib.holds.remove(hold)
    elif b.shelf > 0:
        b.shelf -= 1
    else:
        return "refused %s %s: none available" % (mid, bid)
    due = today + m.days
    m.loans.append([bid, due])
    return "loan %s %s due %s" % (mid, bid, from_ord(due))


def give_back(lib, today, mid, bid):
    m = lib.members.get(mid)
    ln = None
    if m is not None:
        for l in m.loans:
            if l[0] == bid:
                ln = l
    if ln is None:
        return "refused %s %s: not on loan" % (mid, bid)
    m.loans.remove(ln)
    fee = 0
    if today > ln[1]:
        fee = min(500, (today - ln[1]) * m.daily)
    m.owes += fee
    b = lib.books[bid]
    if b.waiting:
        nxt = b.waiting.pop(0)
        lib.holds.append([today + 3, bid, nxt])
    else:
        b.shelf += 1
    r = "returned %s %s" % (mid, bid)
    if fee:
        r += " fee " + money(fee)
    return r


def reserve(lib, today, mid, bid):
    m = lib.members.get(mid)
    if m is None:
        return "refused %s %s: unknown member" % (mid, bid)
    b = lib.books.get(bid)
    if b is None:
        return "refused %s %s: unknown book" % (mid, bid)
    if has_book(m, bid):
        return "refused %s %s: already has it" % (mid, bid)
    if mid in b.waiting:
        return "refused %s %s: already reserved" % (mid, bid)
    if b.shelf > 0:
        return "refused %s %s: copy available" % (mid, bid)
    b.waiting.append(mid)
    return "reserved %s %s position %d" % (mid, bid, len(b.waiting))
