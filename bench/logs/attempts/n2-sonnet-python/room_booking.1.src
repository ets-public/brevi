import sys
import datetime

rooms = {}
bookings = {}  # id -> (room, date, start, end)
for line in sys.stdin.read().split("\n"):
    p = line.split()
    if not p:
        continue
    c = p[0]
    if c == "ROOM":
        rooms[p[1]] = int(p[2])
    elif c == "BOOK":
        bid, date, s, e, people = p[1], p[2], p[3], p[4], int(p[5])
        cands = []
        for name, cap in rooms.items():
            if cap < people:
                continue
            busy = False
            for (r, d, bs, be) in bookings.values():
                if r == name and d == date and bs < e and s < be:
                    busy = True
                    break
            if not busy:
                cands.append((cap, name))
        if cands:
            cands.sort()
            r = cands[0][1]
            bookings[bid] = (r, date, s, e)
            print("%s -> %s" % (bid, r))
        else:
            print("%s rejected" % bid)
    elif c == "CANCEL":
        if p[1] in bookings:
            del bookings[p[1]]
            print("%s cancelled" % p[1])
        else:
            print("%s not found" % p[1])
    elif c == "LIST":
        date = p[1]
        y, mo, d = map(int, date.split("-"))
        print("%s %s" % (date, datetime.date(y, mo, d).strftime("%A")))
        items = sorted((s, r, bid, e) for bid, (r, dd, s, e) in bookings.items() if dd == date)
        if not items:
            print("  no bookings")
        for s, r, bid, e in items:
            print("  %s-%s %s %s" % (s, e, r, bid))
