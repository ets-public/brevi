from graph import dependents_of


def schedule(tasks, k):
    dependents = dependents_of(tasks)
    by = {t.name: t for t in tasks}
    tail = {}

    def tl(n):
        if n not in tail:
            tail[n] = by[n].days + max((tl(s) for s in dependents[n]), default=0)
        return tail[n]

    for t in tasks:
        tl(t.name)

    start = {}
    end = {}
    T = 0
    while len(start) < len(tasks):
        running = sum(1 for n in start if end[n] > T)
        free = k - running
        ready = [t for t in tasks
                 if t.name not in start
                 and all(d in start and end[d] <= T for d in t.deps)]
        ready.sort(key=lambda t: -tl(t.name))
        for t in ready[:max(free, 0)]:
            start[t.name] = T
            end[t.name] = T + t.days
        cands = [end[n] for n in start if end[n] > T]
        if not cands:
            break
        T = min(cands)
    return start, end
