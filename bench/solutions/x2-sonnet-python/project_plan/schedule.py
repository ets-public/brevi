def tails(tasks):
    tail = {}
    for t in sorted(tasks, key=lambda t: -t.end):
        tail[t.name] = t.days + max([tail[u.name] for u in t.users], default=0)
    return tail


def plan(tasks, k):
    tail = tails(tasks)
    by = {t.name: t for t in tasks}
    starts = {}
    ends = {}
    running = []
    T = 0
    while len(ends) < len(tasks):
        for t in [r for r in running if ends[r.name] == T]:
            running.remove(t)
        done = set(n for n in ends if ends[n] <= T)
        ready = [t for t in tasks if t.name not in starts and all(d in done for d in t.deps)]
        ready.sort(key=lambda t: (-tail[t.name], t.idx))
        for t in ready:
            if len(running) >= k:
                break
            starts[t.name] = T
            ends[t.name] = T + t.days
            running.append(t)
        if not running:
            break
        T = min(ends[r.name] for r in running)
    length = max(ends.values()) if ends else 0
    return starts, ends, length
