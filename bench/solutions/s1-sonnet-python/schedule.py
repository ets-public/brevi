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
bad = any(d not in jobs for n in jobs for d in jobs[n][1])
end = {}
start = {}
if not bad:
    state = {}
    def visit(n):
        global bad
        if bad:
            return
        if state.get(n) == 2:
            return
        if state.get(n) == 1:
            bad = True
            return
        state[n] = 1
        s = 0
        for d in jobs[n][1]:
            visit(d)
            if bad:
                return
            s = max(s, end[d])
        start[n] = s
        end[n] = s + jobs[n][0]
        state[n] = 2
    sys.setrecursionlimit(100000)
    for n in jobs:
        visit(n)
if bad:
    print("invalid")
else:
    for n in sorted(jobs, key=lambda x: (start[x], x)):
        print(n, start[n], end[n])
    print("total", max(end.values()) if end else 0)
