import sys
from collections import deque

MIN_CHANGE = 5


def to_min(s):
    h, m = s.split(":")
    return int(h) * 60 + int(m)


def fmt(t):
    return f"{t // 60:02d}:{t % 60:02d}"


def main():
    raw = sys.stdin.read().splitlines()
    sep = raw.index("---")
    train_lines = [l.split() for l in raw[:sep] if l.strip()]
    query_lines = [l.split() for l in raw[sep + 1:] if l.strip()]

    trains = {}   # name -> list of (station, dep, arr)
    at_station = {}  # station -> list of (train, index)
    for parts in train_lines:
        name = parts[0]
        stops = []
        for k in range(1, len(parts) - 1, 2):
            station = parts[k]
            tm = to_min(parts[k + 1])
            stops.append((station, tm, tm))
        # arr for each stop is the time given; dep equals the time given too
        trains[name] = stops
        for j, (station, _, _) in enumerate(stops):
            at_station.setdefault(station, []).append((name, j))

    out = []
    for parts in query_lines:
        frm, to, qtime = parts[0], parts[1], to_min(parts[2])
        if frm == to:
            out.append(f"{frm} {to} {fmt(qtime)} via -")
            continue

        kept = {}  # state (train, index) -> list of (time, count, joined)
        queue = deque()

        def add(state, t, c, s):
            lst = kept.setdefault(state, [])
            for et, ec, es in lst:
                if et <= t and ec <= c and (ec < c or es <= s):
                    return
            lst[:] = [x for x in lst if not (t <= x[0] and c <= x[1] and (c < x[1] or s <= x[2]))]
            lst.append((t, c, s))
            queue.append((state, t, c, s))

        for name, j in at_station.get(frm, []):
            dep = trains[name][j][1]
            if dep >= qtime:
                add((name, j), dep, 1, name)

        while queue:
            state, t, c, s = queue.popleft()
            if (t, c, s) not in kept.get(state, []):
                continue
            name, i = state
            stops = trains[name]
            if i + 1 < len(stops):
                add((name, i + 1), stops[i + 1][2], c, s)
            station = stops[i][0]
            for other, j in at_station.get(station, []):
                if other == name:
                    continue
                dep = trains[other][j][1]
                if dep >= t + MIN_CHANGE:
                    add((other, j), dep, c + 1, s + ">" + other)

        best = None
        for (name, i), labels in kept.items():
            if trains[name][i][0] != to:
                continue
            for lab in labels:
                if best is None or lab < best:
                    best = lab
        if best is None:
            out.append(f"{frm} {to} unreachable")
        else:
            out.append(f"{frm} {to} {fmt(best[0])} via {best[2]}")

    sys.stdout.write("\n".join(out) + ("\n" if out else ""))


if __name__ == "__main__":
    main()
