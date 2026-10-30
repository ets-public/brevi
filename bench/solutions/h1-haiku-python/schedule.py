import sys

def main():
    jobs = {}
    order_in = []
    for line in sys.stdin:
        parts = line.split()
        if not parts:
            continue
        name, dur = parts[0], int(parts[1])
        deps = []
        if len(parts) > 2 and parts[2] == 'after':
            deps = parts[3:]
        jobs[name] = (dur, deps)
        order_in.append(name)
    # validate
    for name in jobs:
        for d in jobs[name][1]:
            if d not in jobs:
                print("invalid")
                return
    # topological sort (Kahn)
    indeg = {n: len(set(jobs[n][1])) for n in jobs}
    children = {n: [] for n in jobs}
    for n in jobs:
        for d in set(jobs[n][1]):
            children[d].append(n)
    ready = [n for n in jobs if indeg[n] == 0]
    topo = []
    while ready:
        n = ready.pop()
        topo.append(n)
        for c in children[n]:
            indeg[c] -= 1
            if indeg[c] == 0:
                ready.append(c)
    if len(topo) != len(jobs):
        print("invalid")
        return
    start = {}
    end = {}
    for n in topo:
        dur, deps = jobs[n]
        s = max((end[d] for d in deps), default=0)
        start[n] = s
        end[n] = s + dur
    rows = sorted(jobs, key=lambda n: (start[n], n))
    out = [f"{n} {start[n]} {end[n]}" for n in rows]
    total = max((end[n] for n in jobs), default=0)
    out.append(f"total {total}")
    sys.stdout.write('\n'.join(out) + '\n')

main()
