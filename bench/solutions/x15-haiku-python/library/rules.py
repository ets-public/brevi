"""Lending rules: loans, returns, fees, holds."""

from catalog import HOLD_DAYS, MAX_FEE, TIERS, Hold, date_text, money


def release_copy(lib, book, today):
    """A copy comes back: hold it for the first on the list, or shelve it."""
    if book.waitlist:
        nxt = book.waitlist.popleft()
        lib.holds.append(Hold(book.id, nxt, today + HOLD_DAYS))
    else:
        book.shelf += 1


def expire_holds(lib, today):
    """Expire every hold whose last day is before today, in order."""
    out = []
    while True:
        due = [h for h in lib.holds if h.last_day < today]
        if not due:
            break
        h = min(due, key=lambda x: (x.last_day, x.book_id))
        lib.holds.remove(h)
        out.append("hold expired %s %s" % (h.member_id, h.book_id))
        book = lib.books[h.book_id]
        if book.waitlist:
            nxt = book.waitlist.popleft()
            lib.holds.append(Hold(book.id, nxt, h.last_day + HOLD_DAYS))
        else:
            book.shelf += 1
    return out


def find_hold(lib, mid, bid):
    for h in lib.holds:
        if h.member_id == mid and h.book_id == bid:
            return h
    return None


def loan(lib, today, mid, bid):
    member = lib.members.get(mid)
    if member is None:
        return "refused %s %s: unknown member" % (mid, bid)
    book = lib.books.get(bid)
    if book is None:
        return "refused %s %s: unknown book" % (mid, bid)
    if member.owes > 200:
        return "refused %s %s: owes %s" % (mid, bid, money(member.owes))
    tier = TIERS[member.tier]
    if len(member.loans) >= tier["limit"]:
        return "refused %s %s: limit" % (mid, bid)
    if bid in member.loans:
        return "refused %s %s: already has it" % (mid, bid)
    hold = find_hold(lib, mid, bid)
    if hold is not None:
        lib.holds.remove(hold)
    elif book.shelf > 0:
        book.shelf -= 1
    else:
        return "refused %s %s: none available" % (mid, bid)
    due = today + tier["days"]
    book.loans[mid] = due
    member.loans.append(bid)
    return "loan %s %s due %s" % (mid, bid, date_text(due))


def ret(lib, today, mid, bid):
    member = lib.members.get(mid)
    book = lib.books.get(bid)
    if member is None or book is None or bid not in member.loans:
        return "refused %s %s: not on loan" % (mid, bid)
    due = book.loans.pop(mid)
    member.loans.remove(bid)
    fee = 0
    late = today - due
    if late > 0:
        rate = TIERS[member.tier]["fee"]
        fee = min(late * rate, MAX_FEE)
        member.owes += fee
    release_copy(lib, book, today)
    line = "returned %s %s" % (mid, bid)
    if fee > 0:
        line += " fee %s" % money(fee)
    return line


def reserve(lib, mid, bid):
    member = lib.members.get(mid)
    if member is None:
        return "refused %s %s: unknown member" % (mid, bid)
    book = lib.books.get(bid)
    if book is None:
        return "refused %s %s: unknown book" % (mid, bid)
    if bid in member.loans:
        return "refused %s %s: already has it" % (mid, bid)
    if mid in book.waitlist or find_hold(lib, mid, bid) is not None:
        return "refused %s %s: already reserved" % (mid, bid)
    if book.shelf > 0:
        return "refused %s %s: copy available" % (mid, bid)
    book.waitlist.append(mid)
    return "reserved %s %s position %d" % (mid, bid, len(book.waitlist))


def pay(lib, mid, amount):
    member = lib.members.get(mid)
    if member is None:
        return "refused %s: unknown member" % mid
    member.owes -= amount
    return "paid %s owes %s" % (mid, money(member.owes))


def status(lib, mid):
    member = lib.members.get(mid)
    if member is None:
        return "refused %s: unknown member" % mid
    loans = " ".join(member.loans) if member.loans else "-"
    return "%s %s owes %s loans: %s" % (mid, member.name, money(member.owes), loans)
