import sys


def main():
    jobs = {}
    order = []
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
        order.append(name)

    for name, (dur, deps) in jobs.items():
        for d in deps:
            if d not in jobs:
                print("invalid")
                return

    # topological order with cycle detection
    indeg = {n: 0 for n in jobs}
    children = {n: [] for n in jobs}
    for n, (dur, deps) in jobs.items():
        for d in set(deps):
            indeg[n] += 1
            children[d].append(n)
    ready = [n for n in jobs if indeg[n] == 0]
    topo = []
    while ready:
        u = ready.pop()
        topo.append(u)
        for v in children[u]:
            indeg[v] -= 1
            if indeg[v] == 0:
                ready.append(v)
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
    out = ["%s %d %d" % (n, start[n], end[n]) for n in rows]
    total = max(end.values()) if end else 0
    out.append("total %d" % total)
    print("\n".join(out))


main()
