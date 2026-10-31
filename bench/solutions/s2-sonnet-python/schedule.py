import sys
jobs = {}
order = []
for line in sys.stdin:
    p = line.split()
    if len(p) < 2:
        continue
    deps = p[3:] if len(p) > 2 and p[2] == 'after' else []
    jobs[p[0]] = (int(p[1]), deps)
    order.append(p[0])
bad = any(d not in jobs for n in jobs for d in jobs[n][1])
start = {}
end = {}
if not bad:
    state = {}
    def visit(n):
        if state.get(n) == 2:
            return True
        if state.get(n) == 1:
            return False
        state[n] = 1
        s = 0
        for d in jobs[n][1]:
            if not visit(d):
                return False
            s = max(s, end[d])
        start[n] = s
        end[n] = s + jobs[n][0]
        state[n] = 2
        return True
    sys.setrecursionlimit(100000)
    for n in jobs:
        if not visit(n):
            bad = True
            break
if bad:
    print("invalid")
else:
    for n in sorted(jobs, key=lambda n: (start[n], n)):
        print(n, start[n], end[n])
    print("total", max(end.values()) if end else 0)
