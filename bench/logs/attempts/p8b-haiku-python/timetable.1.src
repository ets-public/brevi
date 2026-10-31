import sys

sys.setrecursionlimit(20000)


def hm(s):
    h, m = s.split(":")
    return int(h) * 60 + int(m)


def fmt(t):
    return f"{t // 60:02d}:{t % 60:02d}"


text = sys.stdin.read().split("\n")
sep = text.index("---")
train_lines = [ln.split() for ln in text[:sep] if ln.strip()]
query_lines = [ln.split() for ln in text[sep + 1:] if ln.strip()]

# trains: name -> list of (station, time_minutes)
trains = {}
for tok in train_lines:
    name = tok[0]
    stops = []
    for i in range(1, len(tok), 2):
        stops.append((tok[i], hm(tok[i + 1])))
    trains[name] = stops

# boards: station -> list of (train, index)
boards = {}
for name, stops in trains.items():
    for i, (st, _) in enumerate(stops):
        boards.setdefault(st, []).append((name, i))

memo = {}
TO = None


def best(station, ready):
    """Best completion from being at station, able to board at time ready.
    Returns (arrival, count, joined_string, list) or None."""
    key = (station, ready)
    if key in memo:
        return memo[key]
    cands = []
    for name, i in boards.get(station, []):
        stops = trains[name]
        dep = stops[i][1]
        if dep < ready:
            continue
        for j in range(i + 1, len(stops)):
            st_j, arr_j = stops[j]
            if st_j == TO:
                cands.append((arr_j, 1, name, [name]))
            else:
                sub = best(st_j, arr_j + 5)
                if sub is not None:
                    lst = [name] + sub[3]
                    cands.append((sub[0], 1 + sub[1], ">".join(lst), lst))
    res = None
    if cands:
        a, c, j, lst = min(cands, key=lambda x: (x[0], x[1], x[2]))
        res = (a, c, j, lst)
    memo[key] = res
    return res


out = []
for q in query_lines:
    frm, to, t0s = q[0], q[1], q[2]
    t0 = hm(t0s)
    if frm == to:
        out.append(f"{frm} {to} {t0s} via -")
        continue
    TO = to
    memo.clear()
    r = best(frm, t0)
    if r is None:
        out.append(f"{frm} {to} unreachable")
    else:
        out.append(f"{frm} {to} {fmt(r[0])} via {r[2]}")
print("\n".join(out))
