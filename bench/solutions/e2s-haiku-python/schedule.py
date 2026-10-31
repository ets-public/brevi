import sys
from collections import deque


def main():
    jobs = {}
    order_in = []
    for line in sys.stdin.read().splitlines():
        parts = line.split()
        if not parts:
            continue
        name = parts[0]
        dur = int(parts[1])
        deps = []
        if len(parts) > 2 and parts[2] == "after":
            deps = parts[3:]
        jobs[name] = (dur, deps)
        order_in.append(name)

    for name, (dur, deps) in jobs.items():
        for d in deps:
            if d not in jobs:
                print("invalid")
                return

    indeg = {n: len(jobs[n][1]) for n in jobs}
    children = {n: [] for n in jobs}
    for n, (dur, deps) in jobs.items():
        for d in deps:
            children[d].append(n)
    q = deque(n for n in order_in if indeg[n] == 0)
    topo = []
    while q:
        n = q.popleft()
        topo.append(n)
        for c in children[n]:
            indeg[c] -= 1
            if indeg[c] == 0:
                q.append(c)
    if len(topo) < len(jobs):
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
    total = max(end.values(), default=0)
    out.append(f"total {total}")
    sys.stdout.write("\n".join(out) + "\n")


main()
