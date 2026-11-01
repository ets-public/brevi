import sys
import re
import heapq

TIME_RE = re.compile(r'([0-9]{2}):([0-9]{2})')


def parse_time(s):
    m = TIME_RE.fullmatch(s)
    if not m:
        return None
    h, mi = int(m.group(1)), int(m.group(2))
    if h > 23 or mi > 59:
        return None
    return h * 60 + mi


def fmt_time(t):
    return '%02d:%02d' % divmod(t, 60)


def solve(trains, start, goal, t0):
    # A state is (train index, stop index): being on that train at that stop.
    # Its arrival time is fixed, so only the (rides, train ids) label varies.
    best = {}
    heap = []

    def push(ti, k, rides, ids):
        key = (rides, ids)
        st = (ti, k)
        if st not in best or key < best[st]:
            best[st] = key
            heapq.heappush(heap, (rides, ids, ti, k))

    for ti, (tid, stops, times) in enumerate(trains):
        for j, s in enumerate(stops):
            if s == start and times[j] >= t0:
                for k in range(j + 1, len(stops)):
                    push(ti, k, 1, (tid,))

    done = set()
    while heap:
        rides, ids, ti, k = heapq.heappop(heap)
        st = (ti, k)
        if st in done or best[st] != (rides, ids):
            continue
        done.add(st)
        tid, stops, times = trains[ti]
        arr = times[k]
        here = stops[k]
        for ui, (uid, ustops, utimes) in enumerate(trains):
            if ui == ti:
                continue
            for j, s in enumerate(ustops):
                if s == here and utimes[j] >= arr + 2:
                    for kk in range(j + 1, len(ustops)):
                        push(ui, kk, rides + 1, ids + (uid,))
                    break

    result = None
    for (ti, k) in done:
        tid, stops, times = trains[ti]
        if stops[k] != goal:
            continue
        rides, ids = best[(ti, k)]
        cand = (times[k], rides, ids)
        if result is None or cand < result:
            result = cand
    return result


def main():
    trains = []
    train_ids = set()
    stops_known = set()
    trips = 0
    out = []
    for n, raw in enumerate(sys.stdin, 1):
        line = raw.strip()
        if not line:
            continue
        tok = line.split()
        kw = tok[0]
        if kw == 'TRAIN':
            if len(tok) < 6 or len(tok) % 2 != 0:
                out.append('line %d: bad line' % n)
                continue
            stops = tok[2::2]
            times = [parse_time(x) for x in tok[3::2]]
            if any(t is None for t in times):
                out.append('line %d: bad line' % n)
                continue
            increasing = all(times[i] < times[i + 1] for i in range(len(times) - 1))
            if not increasing or len(set(stops)) != len(stops):
                out.append('line %d: bad train' % n)
            elif tok[1] in train_ids:
                out.append('line %d: duplicate train' % n)
            else:
                trains.append((tok[1], stops, times))
                train_ids.add(tok[1])
                stops_known.update(stops)
        elif kw == 'TRIP':
            if len(tok) != 4 or parse_time(tok[3]) is None:
                out.append('line %d: bad line' % n)
                continue
            trips += 1
            frm, to, t0 = tok[1], tok[2], parse_time(tok[3])
            head = 'TRIP %s->%s: ' % (frm, to)
            if frm not in stops_known:
                out.append(head + 'unknown stop ' + frm)
            elif to not in stops_known:
                out.append(head + 'unknown stop ' + to)
            elif frm == to:
                out.append(head + 'already there')
            else:
                res = solve(trains, frm, to, t0)
                if res is None:
                    out.append(head + 'no route')
                else:
                    arr, rides, ids = res
                    out.append(head + 'arrive %s via %s' % (fmt_time(arr), ','.join(ids)))
        else:
            out.append('line %d: bad line' % n)
    out.append('%d trains, %d trips' % (len(trains), trips))
    print('\n'.join(out))


if __name__ == '__main__':
    main()
