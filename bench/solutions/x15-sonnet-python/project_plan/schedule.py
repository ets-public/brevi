def plan(k, tasks):
    order = [t[0] for t in tasks]
    days = {t[0]: t[1] for t in tasks}
    deps = {t[0]: t[2] for t in tasks}
    dependents = {n: [] for n in order}
    for n in order:
        for d in deps[n]:
            dependents[d].append(n)
    tail = {}

    def calc(root):
        stack = [root]
        while stack:
            x = stack[-1]
            if x in tail:
                stack.pop()
                continue
            pend = [d for d in dependents[x] if d not in tail]
            if pend:
                stack.extend(pend)
                continue
            tail[x] = days[x] + max([tail[d] for d in dependents[x]] or [0])
            stack.pop()

    for n in order:
        calc(n)
    pos = {n: i for i, n in enumerate(order)}
    start = {}
    end = {}
    running = []
    done = set()
    t = 0
    while len(done) < len(order):
        for r in [r for r in running if end[r] == t]:
            running.remove(r)
            done.add(r)
        while len(running) < k:
            ready = [n for n in order if n not in start and all(d in done for d in deps[n])]
            if not ready:
                break
            best = max(ready, key=lambda n: (tail[n], -pos[n]))
            start[best] = t
            end[best] = t + days[best]
            running.append(best)
        if len(done) == len(order):
            break
        if not running:
            break
        t = min(end[r] for r in running)
    return start, end, max(end.values())
