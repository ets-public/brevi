import sys
jobs = {}
order = []
for line in sys.stdin:
    p = line.split()
    if not p:
        continue
    deps = p[3:] if len(p) > 2 and p[2] == 'after' else []
    jobs[p[0]] = (int(p[1]), deps)
    order.append(p[0])
bad = False
for n, (d, deps) in jobs.items():
    for x in deps:
        if x not in jobs:
            bad = True
res = {}
if not bad:
    state = {}
    sys.setrecursionlimit(100000)
    def go(n):
        if n in res:
            return True
        if state.get(n) == 1:
            return False
        state[n] = 1
        s = 0
        for x in jobs[n][1]:
            if not go(x):
                return False
            s = max(s, res[x][1])
        res[n] = (s, s + jobs[n][0])
        state[n] = 2
        return True
    for n in jobs:
        if not go(n):
            bad = True
            break
if bad:
    print("invalid")
else:
    for n in sorted(res, key=lambda k: (res[k][0], k)):
        print(n, res[n][0], res[n][1])
    print("total", max([e for _, e in res.values()] or [0]))
