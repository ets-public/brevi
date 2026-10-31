import sys


def to_min(s):
    h, m = s.split(":")
    return int(h) * 60 + int(m)


def fmt(t):
    return f"{t // 60:02d}:{t % 60:02d}"


def main():
    lines = sys.stdin.read().splitlines()
    sep = lines.index("---") if "---" in lines else len(lines)
    trips = []
    for line in lines[:sep]:
        parts = line.split()
        if not parts:
            continue
        name = parts[0]
        stops = []
        for i in range(1, len(parts) - 1, 2):
            stops.append((parts[i], to_min(parts[i + 1])))
        trips.append((name, stops))

    calls_at = {}
    for ti, (_, stops) in enumerate(trips):
        for k, (st, _) in enumerate(stops):
            calls_at.setdefault(st, []).append((ti, k))

    out = []
    for line in lines[sep + 1:]:
        parts = line.split()
        if len(parts) != 3:
            continue
        frm, to, qs = parts
        q = to_min(qs)
        if frm == to:
            out.append(f"{frm} {to} {fmt(q)} via -")
            continue
        out.append(solve(trips, calls_at, frm, to, q))
    sys.stdout.write("\n".join(out) + ("\n" if out else ""))


def solve(trips, calls_at, frm, to, q):
    best = {}  # state (trip index, stop index) -> number of trains
    cur = {}  # state -> set of path tuples, at the current level
    for ti, k in calls_at.get(frm, []):
        name, stops = trips[ti]
        if stops[k][1] < q or k + 1 >= len(stops):
            continue
        for j in range(k + 1, len(stops)):
            cur.setdefault((ti, j), set()).add((name,))

    candidates = []
    n = 1
    while cur:
        level = {}
        work = []
        for s, ps in cur.items():
            if s in best and best[s] < n:
                continue
            best[s] = n
            for p in ps:
                if p not in level.setdefault(s, set()):
                    level[s].add(p)
                    work.append((s, p))
        while work:
            (ti, i), p = work.pop()
            for j in range(i + 1, len(trips[ti][1])):
                ns = (ti, j)
                if ns in best and best[ns] < n:
                    continue
                best[ns] = n
                if p not in level.setdefault(ns, set()):
                    level[ns].add(p)
                    work.append((ns, p))

        nxt = {}
        for (ti, i), ps in level.items():
            name, stops = trips[ti]
            st, arr = stops[i]
            if st == to:
                for p in ps:
                    candidates.append((arr, n, ">".join(p)))
            for ri, k in calls_at.get(st, []):
                if ri == ti:
                    continue
                rname, rstops = trips[ri]
                if rstops[k][1] < arr + 5 or k + 1 >= len(rstops):
                    continue
                for j in range(k + 1, len(rstops)):
                    for p in ps:
                        nxt.setdefault((ri, j), set()).add(p + (rname,))
        cur = nxt
        n += 1

    if not candidates:
        return f"{frm} {to} unreachable"
    arr, _, joined = min(candidates, key=lambda c: (c[0], c[1], c[2]))
    return f"{frm} {to} {fmt(arr)} via {joined}"


if __name__ == "__main__":
    main()
