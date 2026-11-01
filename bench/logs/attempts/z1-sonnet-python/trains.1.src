import sys, re
TM = re.compile(r'^\d\d:\d\d$')

def pt(s):
    if not TM.match(s):
        return None
    h, m = int(s[:2]), int(s[3:])
    if h > 23 or m > 59:
        return None
    return h * 60 + m

trains = {}
stops = set()
nt = 0
nq = 0
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
        names = w[2::2]
        times = [pt(t) for t in w[3::2]]
        if None in times:
            print('line %d: bad line' % n)
            continue
        if len(set(names)) != len(names) or any(times[i] >= times[i + 1] for i in range(len(times) - 1)):
            print('line %d: bad train' % n)
            continue
        if w[1] in trains:
            print('line %d: duplicate train' % n)
            continue
        trains[w[1]] = list(zip(names, times))
        stops.update(names)
        nt += 1
    elif w[0] == 'TRIP':
        t0 = pt(w[3]) if len(w) == 4 else None
        if t0 is None:
            print('line %d: bad line' % n)
            continue
        nq += 1
        fr, to = w[1], w[2]
        pre = 'TRIP %s->%s:' % (fr, to)
        if fr not in stops:
            print('%s unknown stop %s' % (pre, fr)); continue
        if to not in stops:
            print('%s unknown stop %s' % (pre, to)); continue
        if fr == to:
            print('%s already there' % pre); continue
        best = {}
        states = []
        for tid, st in trains.items():
            for i, (s, t) in enumerate(st):
                states.append((t, tid, i))
                if s == fr and t >= t0:
                    k = (1, [tid])
                    if (tid, i) not in best or k < best[(tid, i)]:
                        best[(tid, i)] = k
        states.sort()
        res = None
        for t, tid, i in states:
            if (tid, i) not in best:
                continue
            k = best[(tid, i)]
            st = trains[tid]
            s = st[i][0]
            if s == to:
                c = (t, k[0], k[1])
                if res is None or c < res:
                    res = c
            if i + 1 < len(st):
                key = (tid, i + 1)
                if key not in best or k < best[key]:
                    best[key] = k
            for u, ust in trains.items():
                if u == tid:
                    continue
                for j, (s2, t2) in enumerate(ust):
                    if s2 == s and t2 >= t + 2:
                        nk = (k[0] + 1, k[1] + [u])
                        key = (u, j)
                        if key not in best or nk < best[key]:
                            best[key] = nk
        if res is None:
            print('%s no route' % pre)
        else:
            print('%s arrive %02d:%02d via %s' % (pre, res[0] // 60, res[0] % 60, ','.join(res[2])))
    else:
        print('line %d: bad line' % n)
print('%d trains, %d trips' % (nt, nq))
