import datetime
import sys

WEEKDAYS = ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"]


def to_min(hhmm):
    h, m = hhmm.split(":")
    return int(h) * 60 + int(m)


def main():
    rooms = {}  # name -> capacity
    bookings = {}  # id -> dict(date, start, end, room)
    out = []
    for raw in sys.stdin.read().split("\n"):
        parts = raw.split()
        if not parts:
            continue
        cmd = parts[0]
        if cmd == "ROOM" and len(parts) >= 3:
            rooms[parts[1]] = int(parts[2])
        elif cmd == "BOOK" and len(parts) >= 6:
            bid, date, start, end, people = parts[1], parts[2], parts[3], parts[4], int(parts[5])
            s = to_min(start)
            e = to_min(end)
            candidates = sorted(
                (cap, name) for name, cap in rooms.items() if cap >= people
            )
            chosen = None
            for cap, name in candidates:
                clash = False
                for b in bookings.values():
                    if b["room"] != name or b["date"] != date:
                        continue
                    if s < b["end"] and b["start"] < e:
                        clash = True
                        break
                if not clash:
                    chosen = name
                    break
            if chosen is None:
                out.append("%s rejected" % bid)
            else:
                bookings[bid] = {"date": date, "start": s, "end": e, "room": chosen, "text_start": start, "text_end": end}
                out.append("%s -> %s" % (bid, chosen))
        elif cmd == "CANCEL" and len(parts) >= 2:
            bid = parts[1]
            if bid in bookings:
                del bookings[bid]
                out.append("%s cancelled" % bid)
            else:
                out.append("%s not found" % bid)
        elif cmd == "LIST" and len(parts) >= 2:
            date = parts[1]
            d = datetime.date(*map(int, date.split("-")))
            out.append("%s %s" % (date, WEEKDAYS[d.weekday()]))
            day = [(bid, b) for bid, b in bookings.items() if b["date"] == date]
            if not day:
                out.append("  no bookings")
            else:
                day.sort(key=lambda x: (x[1]["start"], x[1]["room"]))
                for bid, b in day:
                    out.append("  %s-%s %s %s" % (b["text_start"], b["text_end"], b["room"], bid))
    sys.stdout.write("\n".join(out) + ("\n" if out else ""))


if __name__ == "__main__":
    main()
