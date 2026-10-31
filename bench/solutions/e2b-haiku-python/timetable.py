import heapq
import sys


def tm(s):
    h, m = s.split(":")
    return int(h) * 60 + int(m)


def fmt(x):
    return "%02d:%02d" % (x // 60, x % 60)


def solve(trains, frm, to, qt):
    if frm == to:
        return "%s %s %s via -" % (frm, to, fmt(qt))

    # State (train, stop index). Key: (arrival-or-boarding time, trains used, joined list).
    heap = []
    for name, stops in trains.items():
        for j, (st, t) in enumerate(stops[:-1]):
            if st == frm and t >= qt:
                heapq.heappush(heap, (t, 1, name, name, j))

    settled = set()
    while heap:
        arr, cnt, joined, name, i = heapq.heappop(heap)
        if (name, i) in settled:
            continue
        settled.add((name, i))
        stops = trains[name]
        st = stops[i][0]
        if st == to:
            return "%s %s %s via %s" % (frm, to, fmt(arr), joined)

        if i + 1 < len(stops):
            heapq.heappush(heap, (stops[i + 1][1], cnt, joined, name, i + 1))

        for other, ostops in trains.items():
            if other == name:
                continue
            for j, (ost, ot) in enumerate(ostops[:-1]):
                if ost == st and ot >= arr + 5:
                    heapq.heappush(heap, (ot, cnt + 1, joined + ">" + other, other, j))

    return "%s %s unreachable" % (frm, to)


def main():
    lines = sys.stdin.read().splitlines()
    try:
        sep = next(k for k, l in enumerate(lines) if l.strip() == "---")
    except StopIteration:
        sep = len(lines)

    trains = {}
    for line in lines[:sep]:
        tok = line.split()
        if not tok:
            continue
        stops = [(tok[k], tm(tok[k + 1])) for k in range(1, len(tok) - 1, 2)]
        trains[tok[0]] = stops

    out = []
    for line in lines[sep + 1:]:
        tok = line.split()
        if len(tok) != 3:
            continue
        frm, to, t = tok
        out.append(solve(trains, frm, to, tm(t)))
    if out:
        sys.stdout.write("\n".join(out) + "\n")


if __name__ == "__main__":
    main()
