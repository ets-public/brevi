import sys
import datetime

WEEKDAYS = ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"]


def to_min(t):
    h, m = t.split(":")
    return int(h) * 60 + int(m)


def main():
    rooms = {}      # name -> capacity
    bookings = {}   # id -> dict(date, start, end, room, s, e)
    out = []

    for raw in sys.stdin.read().split("\n"):
        parts = raw.split()
        if not parts:
            continue
        cmd = parts[0]
        if cmd == "ROOM":
            rooms[parts[1]] = int(parts[2])
        elif cmd == "BOOK":
            bid, date, start, end, people = parts[1], parts[2], parts[3], parts[4], int(parts[5])
            s, e = to_min(start), to_min(end)
            candidates = []
            for name, cap in rooms.items():
                if cap < people:
                    continue
                clash = False
                for b in bookings.values():
                    if b["date"] == date and b["room"] == name and b["s"] < e and s < b["e"]:
                        clash = True
                        break
                if not clash:
                    candidates.append((cap, name))
            if not candidates:
                out.append("%s rejected" % bid)
            else:
                cap, name = min(candidates)
                bookings[bid] = {"date": date, "start": start, "end": end,
                                 "room": name, "s": s, "e": e}
                out.append("%s -> %s" % (bid, name))
        elif cmd == "CANCEL":
            bid = parts[1]
            if bid in bookings:
                del bookings[bid]
                out.append("%s cancelled" % bid)
            else:
                out.append("%s not found" % bid)
        elif cmd == "LIST":
            date = parts[1]
            d = datetime.date.fromisoformat(date)
            out.append("%s %s" % (date, WEEKDAYS[d.weekday()]))
            items = [(b["s"], b["room"], bid, b) for bid, b in bookings.items() if b["date"] == date]
            if not items:
                out.append("  no bookings")
            for s, room, bid, b in sorted(items, key=lambda x: (x[0], x[1], x[2])):
                out.append("  %s-%s %s %s" % (b["start"], b["end"], room, bid))

    if out:
        sys.stdout.write("\n".join(out) + "\n")


if __name__ == "__main__":
    main()
