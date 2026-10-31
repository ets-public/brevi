import sys

sys.setrecursionlimit(100000)


def to_min(hhmm):
    h, m = hhmm.split(":")
    return int(h) * 60 + int(m)


def fmt(mins):
    return f"{mins // 60:02d}:{mins % 60:02d}"


def main():
    lines = sys.stdin.read().splitlines()
    try:
        sep = lines.index("---")
    except ValueError:
        sep = len(lines)
    trip_lines = lines[:sep]
    query_lines = lines[sep + 1:]

    trips = {}  # train -> (stations, times)
    station_index = {}  # station -> list of (train, idx)
    for line in trip_lines:
        parts = line.split()
        if not parts:
            continue
        train = parts[0]
        stops = parts[1:]
        stations = []
        times = []
        for i in range(0, len(stops) - 1, 2):
            stations.append(stops[i])
            times.append(to_min(stops[i + 1]))
        trips[train] = (stations, times)
        for idx, st in enumerate(stations):
            station_index.setdefault(st, []).append((train, idx))

    memo = {}

    def better(a, b):
        if b is None:
            return True
        if a is None:
            return False
        ka = (a[0], len(a[2]), ">".join(a[2]))
        kb = (b[0], len(b[2]), ">".join(b[2]))
        return ka < kb

    def value(train, k, dest):
        key = (train, k, dest)
        if key in memo:
            return memo[key]
        stations, times = trips[train]
        station = stations[k]
        t = times[k]
        best = None
        if station == dest:
            best = (t, 0, [])
        else:
            if k + 1 < len(stations):
                v = value(train, k + 1, dest)
                if v is not None:
                    cand = (v[0], v[1], list(v[2]))
                    if better(cand, best):
                        best = cand
            for (u, m) in station_index.get(station, []):
                if u == train:
                    continue
                d = trips[u][1][m]
                if d >= t + 5:
                    v = value(u, m, dest)
                    if v is not None:
                        cand = (v[0], v[1] + 1, [u] + list(v[2]))
                        if better(cand, best):
                            best = cand
        memo[key] = best
        return best

    out = []
    for line in query_lines:
        parts = line.split()
        if len(parts) != 3:
            continue
        frm, to, qt = parts
        q = to_min(qt)
        if frm == to:
            out.append(f"{frm} {to} {fmt(q)} via -")
            continue
        best = None
        best_full = None
        for (train, k) in station_index.get(frm, []):
            stations, times = trips[train]
            if times[k] < q:
                continue
            v = value(train, k, to)
            if v is None:
                continue
            full = (v[0], v[1] + 1, [train] + list(v[2]))
            if better(full, best):
                best = full
        if best is None:
            out.append(f"{frm} {to} unreachable")
        else:
            out.append(f"{frm} {to} {fmt(best[0])} via {'>'.join(best[2])}")
    if out:
        sys.stdout.write("\n".join(out) + "\n")


if __name__ == "__main__":
    main()
