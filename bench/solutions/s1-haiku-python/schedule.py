import sys

jobs = {}
order_in = []
for line in sys.stdin:
    p = line.split()
    if not p:
        continue
    name = p[0]
    dur = int(p[1])
    deps = []
    if len(p) > 2 and p[2] == "after":
        deps = p[3:]
    jobs[name] = (dur, deps)
    order_in.append(name)

invalid = False
for name, (dur, deps) in jobs.items():
    for d in deps:
        if d not in jobs:
            invalid = True

if not invalid:
    # Topological processing with cycle detection.
    indeg = {n: 0 for n in jobs}
    children = {n: [] for n in jobs}
    for name, (dur, deps) in jobs.items():
        for d in set(deps):
            indeg[name] += 1
            children[d].append(name)
    ready = [n for n in jobs if indeg[n] == 0]
    done = 0
    start = {}
    end = {}
    while ready:
        n = ready.pop()
        done += 1
        dur, deps = jobs[n]
        start[n] = max((end[d] for d in deps), default=0)
        end[n] = start[n] + dur
        for c in children[n]:
            indeg[c] -= 1
            if indeg[c] == 0:
                ready.append(c)
    if done != len(jobs):
        invalid = True

if invalid:
    print("invalid")
else:
    items = sorted(jobs, key=lambda n: (start[n], n))
    for n in items:
        print(f"{n} {start[n]} {end[n]}")
    total = max(end.values()) if end else 0
    print(f"total {total}")
