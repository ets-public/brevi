import sys


def to_min(s):
    h, m = s.split(":")
    return int(h) * 60 + int(m)


def to_hhmm(t):
    return "%02d:%02d" % (t // 60, t % 60)


def main():
    lines = sys.stdin.read().split("\n")
    sep = lines.index("---") if "---" in lines else len(lines)
    timetable_lines = lines[:sep]
    query_lines = lines[sep + 1:] if sep < len(lines) else []

    trains = {}  # name -> list of (station, minutes)
    for line in timetable_lines:
        parts = line.split()
        if not parts:
            continue
        name = parts[0]
        rest = parts[1:]
        stops = []
        for i in range(0, len(rest) - 1, 2):
            stops.append((rest[i], to_min(rest[i + 1])))
        trains[name] = stops

    # Index of boardable stops: (train, idx) with idx not the last stop.
    def boardable(train):
        stops = trains[train]
        return range(len(stops) - 1)

    out = []
    for line in query_lines:
        parts = line.split()
        if not parts:
            continue
        frm, to, tstr = parts[0], parts[1], parts[2]
        q = to_min(tstr)
        if frm == to:
            out.append("%s %s %s via -" % (frm, to, to_hhmm(q)))
            continue

        labels = {}  # (train, idx) -> (k, joined string)
        queue = []

        def improve(key, k, s):
            cur = labels.get(key)
            if cur is None or (k, s) < cur:
                labels[key] = (k, s)
                queue.append(key)

        for name, stops in trains.items():
            for j in boardable(name):
                st, dep = stops[j]
                if st == frm and dep >= q:
                    improve((name, j), 1, name)

        while queue:
            key = queue.pop()
            name, i = key
            k, s = labels[key]
            stops = trains[name]
            # Ride on the same train to the next stop.
            if i + 1 < len(stops):
                improve((name, i + 1), k, s)
            # Alight here and transfer (needs 5 minutes).
            station, arr = stops[i]
            for other, ostops in trains.items():
                if other == name:
                    continue
                for j in boardable(other):
                    ost, dep = ostops[j]
                    if ost == station and dep >= arr + 5:
                        improve((other, j), k + 1, s + ">" + other)

        best = None
        for (name, i), (k, s) in labels.items():
            station, arr = trains[name][i]
            if station == to:
                cand = (arr, k, s)
                if best is None or cand < best:
                    best = cand

        if best is None:
            out.append("%s %s unreachable" % (frm, to))
        else:
            arr, _, s = best
            out.append("%s %s %s via %s" % (frm, to, to_hhmm(arr), s))

    if out:
        sys.stdout.write("\n".join(out) + "\n")


if __name__ == "__main__":
    main()
