import sys, re


def ptime(s):
    m = re.match(r'^(\d\d):(\d\d)$', s)
    if not m:
        return None
    h, mi = int(m.group(1)), int(m.group(2))
    if h > 23 or mi > 59:
        return None
    return h * 60 + mi


trains = {}
ntr = 0
nq = 0


def plan(src, dst, t0):
    states = []
    for tid, st in trains.items():
        for i, (s, t) in enumerate(st):
            states.append((t, tid, i))
    states.sort()
    best = {}
    for t, tid, i in states:
        s = trains[tid][i][0]
        cands = []
        if s == src and t >= t0:
            cands.append((1, [tid]))
        if i > 0 and (tid, i - 1) in best:
            cands.append(best[(tid, i - 1)])
        for t2, u, j in states:
            if t2 > t - 2:
                break
            if u != tid and trains[u][j][0] == s and (u, j) in best:
                r, l = best[(u, j)]
                cands.append((r + 1, l + [tid]))
        if cands:
            best[(tid, i)] = min(cands)
    res = None
    for (tid, i), (r, l) in best.items():
        if trains[tid][i][0] == dst:
            c = (trains[tid][i][1], r, l)
            if res is None or c < res:
                res = c
    return res


n = 0
for raw in sys.stdin.read().split('\n'):
    n += 1
    w = raw.split()
    if not w:
        continue
    if w[0] == 'TRAIN':
        if len(w) < 6 or len(w) % 2 != 0:
            print('line %d: bad line' % n)
            continue
        ts = [ptime(x) for x in w[3::2]]
        if any(t is None for t in ts):
            print('line %d: bad line' % n)
            continue
        stops = w[2::2]
        if any(ts[i] >= ts[i + 1] for i in range(len(ts) - 1)) or len(set(stops)) != len(stops):
            print('line %d: bad train' % n)
            continue
        if w[1] in trains:
            print('line %d: duplicate train' % n)
            continue
        trains[w[1]] = list(zip(stops, ts))
        ntr += 1
    elif w[0] == 'TRIP':
        if len(w) != 4 or ptime(w[3]) is None:
            print('line %d: bad line' % n)
            continue
        nq += 1
        a, b, t0 = w[1], w[2], ptime(w[3])
        allstops = set(s for st in trains.values() for s, _ in st)
        head = 'TRIP %s->%s:' % (a, b)
        if a not in allstops:
            print('%s unknown stop %s' % (head, a))
        elif b not in allstops:
            print('%s unknown stop %s' % (head, b))
        elif a == b:
            print('%s already there' % head)
        else:
            r = plan(a, b, t0)
            if r is None:
                print('%s no route' % head)
            else:
                print('%s arrive %02d:%02d via %s' % (head, r[0] // 60, r[0] % 60, ','.join(r[2])))
    else:
        print('line %d: bad line' % n)
print('%d trains, %d trips' % (ntr, nq))
