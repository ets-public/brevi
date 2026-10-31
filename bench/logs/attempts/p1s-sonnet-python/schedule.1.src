import sys
jobs = {}
for line in sys.stdin:
    p = line.split()
    if not p:
        continue
    deps = p[3:] if len(p) > 3 and p[2] == 'after' else []
    jobs[p[0]] = (int(p[1]), deps)
bad = any(d not in jobs for _, ds in jobs.values() for d in ds)
start = {}
if not bad:
    state = {}
    sys.setrecursionlimit(100000)
    def go(n):
        if n in start:
            return True
        if state.get(n) == 1:
            return False
        state[n] = 1
        s = 0
        for d in jobs[n][1]:
            if not go(d):
                return False
            s = max(s, start[d] + jobs[d][0])
        start[n] = s
        state[n] = 2
        return True
    for n in jobs:
        if not go(n):
            bad = True
            break
if bad:
    print("invalid")
else:
    order = sorted(jobs, key=lambda n: (start[n], n))
    for n in order:
        print(n, start[n], start[n] + jobs[n][0])
    print("total", max([start[n] + jobs[n][0] for n in jobs] or [0]))
