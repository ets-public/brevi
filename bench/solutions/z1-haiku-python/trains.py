import sys
import re

TIME_RE = re.compile(r"^([01]\d|2[0-3]):([0-5]\d)$")


def parse_time(s):
    m = TIME_RE.match(s)
    if not m:
        return None
    return int(m.group(1)) * 60 + int(m.group(2))


def fmt_time(minutes):
    return "%02d:%02d" % (minutes // 60, minutes % 60)


def plan(trains, frm, to, t0):
    """Return (arrival, rides, ids) for the best plan, or None."""
    at_stop = {}
    total_states = 0
    for ti, (_, stops, times) in enumerate(trains):
        total_states += len(stops)
        for pos, s in enumerate(stops):
            at_stop.setdefault(s, []).append((ti, pos))

    def closure(seeds):
        reach = set()
        for ti, p in seeds:
            n = len(trains[ti][1])
            for q in range(p, n):
                if (ti, q) in reach:
                    break
                reach.add((ti, q))
        return reach

    # layered reachability: layer r = states reached using exactly r rides
    seeds = []
    for ti, (_, stops, times) in enumerate(trains):
        for p, s in enumerate(stops):
            if s == frm and times[p] >= t0:
                seeds.append((ti, p))

    layer_best = []  # (rides, arrival or None)
    r = 1
    while seeds and r <= total_states + 1:
        reach = closure(seeds)
        arr = [trains[ti][2][p] for ti, p in reach if trains[ti][1][p] == to]
        layer_best.append((r, min(arr) if arr else None))
        nxt = []
        for ti, p in reach:
            stop = trains[ti][1][p]
            if stop == to:
                continue
            t = trains[ti][2][p]
            for tj, q in at_stop[stop]:
                if tj != ti and trains[tj][2][q] >= t + 2:
                    nxt.append((tj, q))
        seeds = nxt
        r += 1

    found = [(a, rr) for rr, a in layer_best if a is not None]
    if not found:
        return None
    arrival = min(a for a, _ in found)
    rides = min(rr for a, rr in found if a == arrival)

    memo = {}

    def f(ti, pos, rc):
        key = (ti, pos, rc)
        if key in memo:
            return memo[key]
        stops = trains[ti][1]
        times = trains[ti][2]
        stop = stops[pos]
        if stop == to:
            res = [] if (times[pos] == arrival and rc == rides) else None
            memo[key] = res
            return res
        best = None
        if pos + 1 < len(stops):
            res = f(ti, pos + 1, rc)
            if res is not None:
                best = res
        if rc < rides:
            t = times[pos]
            for tj, q in at_stop[stop]:
                if tj == ti or trains[tj][2][q] < t + 2:
                    continue
                res = f(tj, q, rc + 1)
                if res is not None:
                    cand = [trains[tj][0]] + res
                    if best is None or cand < best:
                        best = cand
        memo[key] = best
        return best

    best_ids = None
    for ti, (tid, stops, times) in enumerate(trains):
        for p, s in enumerate(stops):
            if s == frm and times[p] >= t0:
                res = f(ti, p, 1)
                if res is not None:
                    cand = [tid] + res
                    if best_ids is None or cand < best_ids:
                        best_ids = cand
    if best_ids is None:
        return None
    return arrival, rides, best_ids


def main():
    sys.setrecursionlimit(20000)
    lines = sys.stdin.read().split("\n")
    trains = []
    train_ids = set()
    known_stops = set()
    trips = 0

    for idx, raw in enumerate(lines):
        lineno = idx + 1
        line = raw.rstrip("\r")
        parts = line.split()
        if not parts:
            continue
        kw = parts[0]

        if kw == "TRAIN":
            if len(parts) < 6 or len(parts) % 2 != 0:
                print("line %d: bad line" % lineno)
                continue
            times = []
            ok = True
            for tstr in parts[3::2]:
                tv = parse_time(tstr)
                if tv is None:
                    ok = False
                    break
                times.append(tv)
            if not ok:
                print("line %d: bad line" % lineno)
                continue
            tid = parts[1]
            stops = parts[2::2]
            increasing = all(times[i] < times[i + 1] for i in range(len(times) - 1))
            unique = len(set(stops)) == len(stops)
            if not (increasing and unique):
                print("line %d: bad train" % lineno)
                continue
            if tid in train_ids:
                print("line %d: duplicate train" % lineno)
                continue
            trains.append((tid, stops, times))
            train_ids.add(tid)
            known_stops.update(stops)

        elif kw == "TRIP":
            if len(parts) != 4:
                print("line %d: bad line" % lineno)
                continue
            t0 = parse_time(parts[3])
            if t0 is None:
                print("line %d: bad line" % lineno)
                continue
            trips += 1
            frm, to = parts[1], parts[2]
            head = "TRIP %s->%s" % (frm, to)
            if frm not in known_stops:
                print("%s: unknown stop %s" % (head, frm))
            elif to not in known_stops:
                print("%s: unknown stop %s" % (head, to))
            elif frm == to:
                print("%s: already there" % head)
            else:
                res = plan(trains, frm, to, t0)
                if res is None:
                    print("%s: no route" % head)
                else:
                    arrival, _, ids = res
                    print("%s: arrive %s via %s" % (head, fmt_time(arrival), ",".join(ids)))

        else:
            print("line %d: bad line" % lineno)

    print("%d trains, %d trips" % (len(trains), trips))


if __name__ == "__main__":
    main()
