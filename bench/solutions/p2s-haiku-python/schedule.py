import sys

jobs = {}
for line in sys.stdin:
    parts = line.split()
    if not parts:
        continue
    name, dur = parts[0], int(parts[1])
    deps = []
    if len(parts) > 2 and parts[2] == 'after':
        deps = parts[3:]
    jobs[name] = (dur, deps)


def invalid():
    print("invalid")
    sys.exit(0)


for name, (dur, deps) in jobs.items():
    for d in deps:
        if d not in jobs:
            invalid()

start = {}
end = {}
state = {}


def visit(n):
    if state.get(n) == 2:
        return
    if state.get(n) == 1:
        invalid()
    state[n] = 1
    dur, deps = jobs[n]
    s = 0
    for d in deps:
        visit(d)
        s = max(s, end[d])
    start[n] = s
    end[n] = s + dur
    state[n] = 2


for n in jobs:
    visit(n)

rows = sorted(jobs, key=lambda n: (start[n], n))
for n in rows:
    print(n, start[n], end[n])
print("total", max(end.values()) if end else 0)
