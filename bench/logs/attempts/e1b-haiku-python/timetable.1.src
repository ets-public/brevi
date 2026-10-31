import sys


def tm(text):
    h, m = text.split(':')
    return int(h) * 60 + int(m)


def fmt(t):
    return '%02d:%02d' % (t // 60, t % 60)


def solve(trains, fr, to, qt):
    best = [None]

    def dfs(name, i, t, route, used):
        stops = trains[name]
        station = stops[i][0]
        if station == to:
            key = (t, len(route), '>'.join(route))
            if best[0] is None or key < best[0]:
                best[0] = key
            return
        if best[0] is not None and (t, len(route)) > best[0][:2]:
            return
        # stay on the same train to the next stop
        if i + 1 < len(stops):
            dfs(name, i + 1, stops[i + 1][1], route, used)
        # change trains at this station
        for other, ostops in trains.items():
            if other in used:
                continue
            for k, (st, dep) in enumerate(ostops):
                if st == station and dep >= t + 5:
                    dfs(other, k, dep, route + [other], used | {other})

    for name, stops in trains.items():
        for k, (st, dep) in enumerate(stops):
            if st == fr and dep >= qt:
                dfs(name, k, dep, [name], frozenset([name]))
    return best[0]


def main():
    trains = {}
    queries = []
    in_queries = False
    for raw in sys.stdin.read().split('\n'):
        line = raw.strip()
        if line == '---':
            in_queries = True
            continue
        if not line:
            continue
        if in_queries:
            queries.append(line.split())
        else:
            toks = line.split()
            rest = toks[1:]
            trains[toks[0]] = [(rest[i], tm(rest[i + 1])) for i in range(0, len(rest) - 1, 2)]

    out = []
    for fr, to, hhmm in queries:
        qt = tm(hhmm)
        if fr == to:
            out.append(f'{fr} {to} {fmt(qt)} via -')
            continue
        best = solve(trains, fr, to, qt)
        if best is None:
            out.append(f'{fr} {to} unreachable')
        else:
            out.append(f'{fr} {to} {fmt(best[0])} via {best[2]}')
    if out:
        sys.stdout.write('\n'.join(out) + '\n')


main()
