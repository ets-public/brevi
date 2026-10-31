from graph import dependents


def tails(tasks, order):
    by = dict((t.name, t) for t in tasks)
    dep = dependents(tasks)
    tail = {}
    for n in reversed(order):
        tail[n] = by[n].days + max([tail[s] for s in dep[n]] or [0])
    return tail


def schedule(tasks, order, K):
    """Plan with K workers. Returns (start, end, length)."""
    by = dict((t.name, t) for t in tasks)
    tail = tails(tasks, order)
    index = dict((t.name, i) for i, t in enumerate(tasks))
    start = {}
    end = {}
    running = []
    T = 0
    while len(start) < len(tasks):
        # finish tasks that end at T (they free their workers)
        running = [n for n in running if end[n] > T]
        ready = [t.name for t in tasks
                 if t.name not in start
                 and all(d in end and end[d] <= T for d in t.deps)]
        ready.sort(key=lambda n: (-tail[n], index[n]))
        for n in ready:
            if len(running) >= K:
                break
            start[n] = T
            end[n] = T + by[n].days
            running.append(n)
        if len(start) == len(tasks):
            break
        T = min(end[n] for n in running)
    L = max(end.values()) if end else 0
    return start, end, L
