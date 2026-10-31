import sys

jobs = {}
for line in sys.stdin:
    parts = line.split()
    if not parts:
        continue
    name, dur = parts[0], int(parts[1])
    deps = []
    if len(parts) > 2:
        rest = parts[2:]
        if rest[0] == "after":
            rest = rest[1:]
        deps = rest
    jobs[name] = (dur, deps)


def solve():
    for name, (dur, deps) in jobs.items():
        for d in deps:
            if d not in jobs:
                return None
    start = {}
    end = {}
    state = {}

    def visit(n):
        if state.get(n) == 2:
            return True
        if state.get(n) == 1:
            return False
        state[n] = 1
        dur, deps = jobs[n]
        s = 0
        for d in deps:
            if not visit(d):
                return False
            s = max(s, end[d])
        start[n] = s
        end[n] = s + dur
        state[n] = 2
        return True

    for n in jobs:
        if not visit(n):
            return None
    return start, end


res = solve()
if res is None:
    print("invalid")
else:
    start, end = res
    order = sorted(jobs, key=lambda n: (start[n], n))
    for n in order:
        print(n, start[n], end[n])
    print("total", max(end.values()) if end else 0)
