import sys


def hm(s):
    h, m = s.split(':')
    return int(h) * 60 + int(m)


def fmt(t):
    return f"{t // 60:02d}:{t % 60:02d}"


def main():
    trains = {}
    queries = []
    sec = 0
    for raw in sys.stdin.read().splitlines():
        line = raw.strip()
        if not line:
            continue
        if line == '---':
            sec = 1
            continue
        if sec == 0:
            tok = line.split()
            stops = []
            for i in range(1, len(tok), 2):
                stops.append((tok[i], hm(tok[i + 1])))
            trains[tok[0]] = stops
        else:
            f, t, tm = line.split()
            queries.append((f, t, hm(tm)))

    by_station = {}
    states = []
    for name, stops in trains.items():
        for i, (st, tm) in enumerate(stops):
            by_station.setdefault(st, []).append((name, i))
            states.append((tm, name, i))
    states.sort()

    out = []
    for f, t, t0 in queries:
        if f == t:
            out.append(f"{f} {t} {fmt(t0)} via -")
            continue

        labels = {}

        def upd(key, lab):
            if key not in labels or lab < labels[key]:
                labels[key] = lab

        for name, stops in trains.items():
            for i, (st, dep) in enumerate(stops):
                if st == f and i < len(stops) - 1 and dep >= t0:
                    upd((name, i), (1, name))

        best = None
        for tm, name, i in states:
            key = (name, i)
            if key not in labels:
                continue
            n, joined = labels[key]
            stops = trains[name]
            st = stops[i][0]
            if st == t:
                cand = (tm, n, joined)
                if best is None or cand < best:
                    best = cand
                continue
            if i + 1 < len(stops):
                upd((name, i + 1), (n, joined))
            for other, k in by_station.get(st, []):
                if other == name:
                    continue
                if k == len(trains[other]) - 1:
                    continue
                if trains[other][k][1] >= tm + 5:
                    upd((other, k), (n + 1, joined + '>' + other))

        if best is None:
            out.append(f"{f} {t} unreachable")
        else:
            out.append(f"{f} {t} {fmt(best[0])} via {best[2]}")

    print('\n'.join(out))


if __name__ == '__main__':
    main()
