import heapq
import sys


def to_min(s):
    h, m = s.split(':')
    return int(h) * 60 + int(m)


def fmt(t):
    return '%02d:%02d' % (t // 60, t % 60)


def main():
    lines = [l.strip() for l in sys.stdin.read().split('\n')]
    try:
        sep = lines.index('---')
    except ValueError:
        sep = len(lines)
    train_lines = [l for l in lines[:sep] if l]
    query_lines = [l for l in lines[sep + 1:] if l]

    trains = {}  # name -> list of (station, minutes)
    for l in train_lines:
        parts = l.split()
        name = parts[0]
        stops = []
        rest = parts[1:]
        for j in range(0, len(rest) - 1, 2):
            stops.append((rest[j], to_min(rest[j + 1])))
        trains[name] = stops

    # boardable[station] -> list of (train, index) where a departure exists
    boardable = {}
    for name, stops in trains.items():
        for i, (st, _) in enumerate(stops[:-1]):
            boardable.setdefault(st, []).append((name, i))

    out = []
    for q in query_lines:
        parts = q.split()
        if len(parts) != 3:
            continue
        frm, to, qt_s = parts
        qt = to_min(qt_s)
        if frm == to:
            out.append('%s %s %s via -' % (frm, to, fmt(qt)))
            continue

        best = {}
        heap = []
        for name, stops in trains.items():
            for i, (st, dep) in enumerate(stops[:-1]):
                if st == frm and dep >= qt:
                    key = (dep, 1, name)
                    state = (name, i)
                    heapq.heappush(heap, (dep, 1, name, name, i))
                    best[state] = key

        while heap:
            t, n, path, T, i = heapq.heappop(heap)
            if best.get((T, i)) != (t, n, path):
                continue
            stops = trains[T]
            # ride to next stop
            if i + 1 < len(stops):
                nt = stops[i + 1][1]
                key = (nt, n, path)
                state = (T, i + 1)
                if state not in best or key < best[state]:
                    best[state] = key
                    heapq.heappush(heap, (nt, n, path, T, i + 1))
            # transfer at current station
            here = stops[i][0]
            for U, k in boardable.get(here, []):
                if U == T:
                    continue
                dep = trains[U][k][1]
                if dep >= t + 5:
                    key = (dep, n + 1, path + '>' + U)
                    state = (U, k)
                    if state not in best or key < best[state]:
                        best[state] = key
                        heapq.heappush(heap, (dep, n + 1, path + '>' + U, U, k))

        cands = []
        for (T, i), key in best.items():
            stops = trains[T]
            if stops[i][0] == to:
                cands.append((stops[i][1], key[1], key[2]))
        # Also consider alighting at the final stop of a train (state not
        # produced by boarding, reached only by riding) -- covered by ride.
        if not cands:
            out.append('%s %s unreachable' % (frm, to))
            continue
        arr, _, path = min(cands)
        out.append('%s %s %s via %s' % (frm, to, fmt(arr), path))

    if out:
        sys.stdout.write('\n'.join(out) + '\n')


if __name__ == '__main__':
    main()
