from datetime import timedelta

from catalog import TIERS, MAX_FEE, HOLD_DAYS, Hold, money, fmt_date


def expire_holds(lib, today):
    out = []
    while True:
        expired = []
        for b in lib.books.values():
            for h in b.holds:
                if h.last < today:
                    expired.append((h.last, b.id, b, h))
        if not expired:
            break
        expired.sort(key=lambda e: (e[0], e[1]))
        last, bid, b, h = expired[0]
        b.holds.remove(h)
        out.append("hold expired %s %s" % (h.member_id, bid))
        if b.waitlist:
            nxt = b.waitlist.pop(0)
            b.holds.append(Hold(nxt, last + timedelta(days=HOLD_DAYS)))
        else:
            b.shelf += 1
    return out


def loan(lib, today, mid, bid):
    m = lib.members.get(mid)
    b = lib.books.get(bid)
    if m is None:
        return "refused %s %s: unknown member" % (mid, bid)
    if b is None:
        return "refused %s %s: unknown book" % (mid, bid)
    if m.owes > 200:
        return "refused %s %s: owes %s" % (mid, bid, money(m.owes))
    tier = TIERS[m.tier]
    if len(m.loans) >= tier['limit']:
        return "refused %s %s: limit" % (mid, bid)
    if any(rec[0] == bid for rec in m.loans):
        return "refused %s %s: already has it" % (mid, bid)
    hold = None
    for h in b.holds:
        if h.member_id == mid:
            hold = h
            break
    if hold is None and b.shelf <= 0:
        return "refused %s %s: none available" % (mid, bid)
    if hold is not None:
        b.holds.remove(hold)
    else:
        b.shelf -= 1
    due = today + timedelta(days=tier['days'])
    m.loans.append([bid, due])
    return "loan %s %s due %s" % (mid, bid, fmt_date(due))


def ret(lib, today, mid, bid):
    m = lib.members.get(mid)
    b = lib.books.get(bid)
    rec = None
    if m is not None:
        for r in m.loans:
            if r[0] == bid:
                rec = r
                break
    if rec is None or b is None:
        return "refused %s %s: not on loan" % (mid, bid)
    m.loans.remove(rec)
    tier = TIERS[m.tier]
    late = (today - rec[1]).days
    fee = 0
    if late > 0:
        fee = min(late * tier['fee'], MAX_FEE)
        m.owes += fee
    if b.waitlist:
        nxt = b.waitlist.pop(0)
        b.holds.append(Hold(nxt, today + timedelta(days=HOLD_DAYS)))
    else:
        b.shelf += 1
    msg = "returned %s %s" % (mid, bid)
    if fee:
        msg += " fee %s" % money(fee)
    return msg


def reserve(lib, today, mid, bid):
    m = lib.members.get(mid)
    b = lib.books.get(bid)
    if m is None:
        return "refused %s %s: unknown member" % (mid, bid)
    if b is None:
        return "refused %s %s: unknown book" % (mid, bid)
    if any(rec[0] == bid for rec in m.loans):
        return "refused %s %s: already has it" % (mid, bid)
    if mid in b.waitlist or any(h.member_id == mid for h in b.holds):
        return "refused %s %s: already reserved" % (mid, bid)
    if b.shelf > 0:
        return "refused %s %s: copy available" % (mid, bid)
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
    books = ' '.join(rec[0] for rec in m.loans) or '-'
    return "%s %s owes %s loans: %s" % (mid, m.name, money(m.owes), books)
