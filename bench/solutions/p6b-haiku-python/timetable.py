import sys
import heapq


def hm(s):
    h, m = s.split(':')
    return int(h) * 60 + int(m)


def fmt(t):
    return f"{t // 60:02d}:{t % 60:02d}"


def solve(trips, frm, to):
    # State: (time, trains used, joined train names, trip index, stop position).
    # Lexicographic order on (time, count, joined string) gives the preference.
    heap = []
    for x, (name, stops, times) in enumerate(trips):
        for j, s in enumerate(stops):
            if s == frm and times[j] >= qt_global[0] and j + 1 < len(stops):
                heapq.heappush(heap, (times[j + 1], 1, name, x, j + 1))
    done = set()
    while heap:
        t, k, joined, x, pos = heapq.heappop(heap)
        if (x, pos) in done:
            continue
        done.add((x, pos))
        name, stops, times = trips[x]
        station = stops[pos]
        if station == to:
            return f"{frm} {to} {fmt(t)} via {joined}"
        if pos + 1 < len(stops):
            heapq.heappush(heap, (times[pos + 1], k, joined, x, pos + 1))
        for y, (yname, ystops, ytimes) in enumerate(trips):
            if y == x:
                continue
            for j, s in enumerate(ystops):
                if s == station and ytimes[j] >= t + 5 and j + 1 < len(ystops):
                    heapq.heappush(heap, (ytimes[j + 1], k + 1, joined + '>' + yname, y, j + 1))
    return f"{frm} {to} unreachable"


qt_global = [0]


def main():
    raw = sys.stdin.read().splitlines()
    try:
        sep = raw.index('---')
    except ValueError:
        sep = len(raw)
    trips = []
    for line in raw[:sep]:
        tk = line.split()
        if not tk:
            continue
        trips.append((tk[0], tk[1::2], [hm(x) for x in tk[2::2]]))
    out = []
    for line in raw[sep + 1:]:
        tk = line.split()
        if not tk:
            continue
        frm, to, qt = tk[0], tk[1], hm(tk[2])
        if frm == to:
            out.append(f"{frm} {to} {fmt(qt)} via -")
            continue
        qt_global[0] = qt
        out.append(solve(trips, frm, to))
    print('\n'.join(out))


main()
