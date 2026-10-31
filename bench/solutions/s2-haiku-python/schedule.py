import sys

jobs = {}
for line in sys.stdin.read().splitlines():
    p = line.split()
    if not p:
        continue
    name = p[0]
    dur = int(p[1])
    deps = []
    if len(p) > 2 and p[2] == "after":
        deps = p[3:]
    jobs[name] = (dur, deps)

invalid = False
for name, (dur, deps) in jobs.items():
    for d in deps:
        if d not in jobs:
            invalid = True

if not invalid:
    state = {}
    end = {}

    def finish(n):
        global invalid
        if invalid:
            return 0
        if state.get(n) == 2:
            return end[n]
        if state.get(n) == 1:
            invalid = True
            return 0
        state[n] = 1
        dur, deps = jobs[n]
        start = 0
        for d in deps:
            start = max(start, finish(d))
            if invalid:
                return 0
        end[n] = start + dur
        state[n] = 2
        return end[n]

    for n in jobs:
        finish(n)
        if invalid:
            break

if invalid:
    print("invalid")
else:
    rows = []
    for n, (dur, deps) in jobs.items():
        rows.append((end[n] - dur, n, end[n]))
    rows.sort()
    for s, n, e in rows:
        print(n, s, e)
    total = max((e for _, _, e in rows), default=0)
    print("total", total)
